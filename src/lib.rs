#![forbid(unsafe_code)]

use std::collections;
use std::vec;

/// Fixed-point limit price. The market defines the scale/tick size.
pub type Price = i64;

/// Order quantity cannot be negative, so it is unsigned.
pub type Quantity = u64;

/// Exchange-generated order identifier.
pub type OrderId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Client trading intent. The exchange assigns the order ID on submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewOrder {
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trade {
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitResult {
    pub order_id: OrderId,
    pub trades: vec::Vec<Trade>,
    pub remaining_quantity: Quantity,
    pub resting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookLevel {
    pub price: Price,
    pub total_quantity: Quantity,
    pub order_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookSnapshot {
    /// Highest bid first.
    pub bids: vec::Vec<BookLevel>,
    /// Lowest ask first.
    pub asks: vec::Vec<BookLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RestingOrder {
    id: OrderId,
    remaining_quantity: Quantity,
}

/// In-memory limit order book for one instrument.
///
/// Matching uses price-time priority:
/// 1. Best price first.
/// 2. FIFO within the same price level.
/// 3. The resting maker's price is the execution price.
/// 4. Unfilled quantity remains resting on the book.
///
/// `BTreeMap` keeps price levels ordered and `VecDeque` preserves FIFO order.
/// The book intentionally has one mutable owner; production scale comes from
/// assigning different instruments to independent matcher shards.
#[derive(Debug)]
pub struct OrderBook {
    bids: collections::BTreeMap<Price, collections::VecDeque<RestingOrder>>,
    asks: collections::BTreeMap<Price, collections::VecDeque<RestingOrder>>,
    next_order_id: OrderId,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: collections::BTreeMap::new(),
            asks: collections::BTreeMap::new(),
            next_order_id: 1,
        }
    }

    pub fn submit(&mut self, order: NewOrder) -> eyre::Result<SubmitResult> {
        // TODO: might move validation to the gateway for production to reduce the work here
        Self::validate_new_order(&order)?;
        let order_id = self.allocate_order_id()?;

        let mut remaining_quantity = order.quantity;
        let mut trades = vec::Vec::new();

        self.match_order(
            order_id,
            order.side,
            order.price,
            &mut remaining_quantity,
            &mut trades,
        )?;

        let resting = remaining_quantity > 0;
        if resting {
            self.add_resting_order(order.side, order.price, order_id, remaining_quantity);
        }

        Ok(SubmitResult {
            order_id,
            trades,
            remaining_quantity,
            resting,
        })
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    pub fn snapshot(&self) -> eyre::Result<BookSnapshot> {
        let mut bids = vec::Vec::with_capacity(self.bids.len());
        let mut asks = vec::Vec::with_capacity(self.asks.len());

        for (price, orders) in self.bids.iter().rev() {
            bids.push(Self::summarize_level(*price, orders)?);
        }

        for (price, orders) in &self.asks {
            asks.push(Self::summarize_level(*price, orders)?);
        }

        Ok(BookSnapshot { bids, asks })
    }

    fn validate_new_order(order: &NewOrder) -> eyre::Result<()> {
        eyre::ensure!(order.price > 0, "price must be greater than zero");
        eyre::ensure!(order.quantity > 0, "quantity must be greater than zero");
        Ok(())
    }

    fn allocate_order_id(&mut self) -> eyre::Result<OrderId> {
        let order_id = self.next_order_id;
        self.next_order_id = self
            .next_order_id
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("order ID space exhausted"))?;
        Ok(order_id)
    }

    fn match_order(
        &mut self,
        taker_order_id: OrderId,
        side: Side,
        limit_price: Price,
        remaining_quantity: &mut Quantity,
        trades: &mut vec::Vec<Trade>,
    ) -> eyre::Result<()> {
        // Select the opposite book once. The only side-dependent work left in the
        // loop is choosing its best end and checking whether that price crosses.
        let opposite_levels = match side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

        while *remaining_quantity > 0 {
            // Select the best level and check the limit in one side-dependent
            // branch. `side` is constant for the whole submission, making this
            // branch highly predictable while keeping one shared matching loop.
            let mut best_level = match side {
                Side::Buy => match opposite_levels.first_entry() {
                    Some(level) if *level.key() <= limit_price => level,
                    Some(_) | None => break,
                },
                Side::Sell => match opposite_levels.last_entry() {
                    Some(level) if *level.key() >= limit_price => level,
                    Some(_) | None => break,
                },
            };

            let maker_price = *best_level.key();

            let (trade, level_is_empty) = {
                let level = best_level.get_mut();
                let maker = level
                    .front_mut()
                    .ok_or_else(|| eyre::eyre!("best price level is empty"))?;

                // `min` guarantees both following subtractions are safe for u64.
                let executed_quantity = (*remaining_quantity).min(maker.remaining_quantity);
                maker.remaining_quantity -= executed_quantity;
                *remaining_quantity -= executed_quantity;

                let trade = Trade {
                    maker_order_id: maker.id,
                    taker_order_id,
                    price: maker_price,
                    quantity: executed_quantity,
                };

                if maker.remaining_quantity == 0 {
                    level.pop_front();
                }

                (trade, level.is_empty())
            };

            trades.push(trade);

            if level_is_empty {
                // `remove_entry` removes the level through the occupied entry we
                // already hold, avoiding another O(log price_levels) lookup.
                best_level.remove_entry();
            }
        }

        Ok(())
    }

    fn add_resting_order(
        &mut self,
        side: Side,
        price: Price,
        order_id: OrderId,
        remaining_quantity: Quantity,
    ) {
        let resting_order = RestingOrder {
            id: order_id,
            remaining_quantity,
        };

        let levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        levels
            .entry(price)
            .or_insert_with(collections::VecDeque::new)
            .push_back(resting_order);
    }

    fn summarize_level(
        price: Price,
        orders: &collections::VecDeque<RestingOrder>,
    ) -> eyre::Result<BookLevel> {
        let mut total_quantity: Quantity = 0;

        for order in orders {
            total_quantity = total_quantity
                .checked_add(order.remaining_quantity)
                .ok_or_else(|| eyre::eyre!("quantity overflow at price {price}"))?;
        }

        Ok(BookLevel {
            price,
            total_quantity,
            order_count: orders.len(),
        })
    }
}
