#[test]
fn matching_engine_assigns_monotonic_order_ids() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    let first = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 99_i64,
        quantity: 1_u64,
    })?;
    let second = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 101_i64,
        quantity: 1_u64,
    })?;

    assert_eq!(first.order_id, 1_u64);
    assert_eq!(second.order_id, 2_u64);
    Ok(())
}

#[test]
fn matches_best_price_before_worse_price() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    let worse_ask = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 101_i64,
        quantity: 1_u64,
    })?;
    let best_ask = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 1_u64,
    })?;

    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 101_i64,
        quantity: 1_u64,
    })?;

    assert_eq!(buy.trades.len(), 1);
    assert_eq!(buy.trades[0].maker_order_id, best_ask.order_id);
    assert_eq!(buy.trades[0].price, 100_i64);
    assert_ne!(buy.trades[0].maker_order_id, worse_ask.order_id);
    assert_eq!(book.best_ask(), Some(101_i64));
    Ok(())
}

#[test]
fn preserves_fifo_priority_at_same_price() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    let first = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 5_u64,
    })?;
    let second = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 5_u64,
    })?;

    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 7_u64,
    })?;

    assert_eq!(buy.trades.len(), 2);
    assert_eq!(buy.trades[0].maker_order_id, first.order_id);
    assert_eq!(buy.trades[0].quantity, 5_u64);
    assert_eq!(buy.trades[1].maker_order_id, second.order_id);
    assert_eq!(buy.trades[1].quantity, 2_u64);

    let snapshot = book.snapshot()?;
    assert_eq!(snapshot.asks[0].total_quantity, 3_u64);
    Ok(())
}

#[test]
fn partially_filled_taker_rests_with_remainder() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 2_u64,
    })?;

    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 5_u64,
    })?;

    assert_eq!(buy.remaining_quantity, 3_u64);
    assert!(buy.resting);
    assert_eq!(book.best_bid(), Some(100_i64));

    let snapshot = book.snapshot()?;
    assert_eq!(snapshot.bids[0].total_quantity, 3_u64);
    Ok(())
}

#[test]
fn non_crossing_orders_stay_on_book() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 99_i64,
        quantity: 4_u64,
    })?;
    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 101_i64,
        quantity: 6_u64,
    })?;

    assert_eq!(book.best_bid(), Some(99_i64));
    assert_eq!(book.best_ask(), Some(101_i64));

    let snapshot = book.snapshot()?;
    assert_eq!(snapshot.bids[0].total_quantity, 4_u64);
    assert_eq!(snapshot.asks[0].total_quantity, 6_u64);
    Ok(())
}

#[test]
fn sell_taker_matches_highest_bid_first() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 99_i64,
        quantity: 1_u64,
    })?;
    let best_bid = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 101_i64,
        quantity: 1_u64,
    })?;

    let sell = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 99_i64,
        quantity: 1_u64,
    })?;

    assert_eq!(sell.trades.len(), 1);
    assert_eq!(sell.trades[0].maker_order_id, best_bid.order_id);
    assert_eq!(sell.trades[0].price, 101_i64);
    assert_eq!(book.best_bid(), Some(99_i64));
    Ok(())
}

#[test]
fn trade_uses_resting_maker_price() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 1_u64,
    })?;

    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 110_i64,
        quantity: 1_u64,
    })?;

    assert_eq!(buy.trades[0].price, 100_i64);
    Ok(())
}

#[test]
fn rejects_invalid_price_and_zero_quantity() {
    let mut book = matching_engine::OrderBook::new();

    let zero_price = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 0_i64,
        quantity: 1_u64,
    });
    assert!(zero_price.is_err());

    let negative_price = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: -1_i64,
        quantity: 1_u64,
    });
    assert!(negative_price.is_err());

    let zero_quantity = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 0_u64,
    });
    assert!(zero_quantity.is_err());
}

#[test]
fn snapshot_aggregates_price_levels() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 3_u64,
    })?;
    book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 4_u64,
    })?;

    let snapshot = book.snapshot()?;

    assert_eq!(snapshot.bids.len(), 1);
    assert_eq!(snapshot.bids[0].price, 100_i64);
    assert_eq!(snapshot.bids[0].total_quantity, 7_u64);
    assert_eq!(snapshot.bids[0].order_count, 2);
    Ok(())
}

#[test]
fn supports_quantities_above_i64_max() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();
    let quantity = (i64::MAX as u64) + 1_u64;

    let result = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity,
    })?;

    assert_eq!(result.remaining_quantity, quantity);

    let snapshot = book.snapshot()?;
    assert_eq!(snapshot.bids[0].total_quantity, quantity);
    Ok(())
}

#[test]
fn buy_taker_sweeps_multiple_ask_levels_in_price_order() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    let ask_101 = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 101_i64,
        quantity: 1_u64,
    })?;
    let ask_100 = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 100_i64,
        quantity: 1_u64,
    })?;
    let ask_102 = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 102_i64,
        quantity: 1_u64,
    })?;

    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 102_i64,
        quantity: 3_u64,
    })?;

    assert_eq!(buy.trades.len(), 3);
    assert_eq!(buy.trades[0].maker_order_id, ask_100.order_id);
    assert_eq!(buy.trades[0].price, 100_i64);
    assert_eq!(buy.trades[1].maker_order_id, ask_101.order_id);
    assert_eq!(buy.trades[1].price, 101_i64);
    assert_eq!(buy.trades[2].maker_order_id, ask_102.order_id);
    assert_eq!(buy.trades[2].price, 102_i64);
    assert_eq!(buy.remaining_quantity, 0_u64);
    assert!(!buy.resting);
    assert_eq!(book.best_ask(), None);

    Ok(())
}

#[test]
fn invalid_order_does_not_consume_order_id() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    assert!(
        book.submit(matching_engine::NewOrder {
            side: matching_engine::Side::Buy,
            price: 0_i64,
            quantity: 1_u64,
        })
        .is_err()
    );

    let accepted = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 100_i64,
        quantity: 1_u64,
    })?;

    assert_eq!(accepted.order_id, 1_u64);
    Ok(())
}

#[test]
fn sell_taker_sweeps_multiple_bid_levels_in_price_order() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    for price in [99_i64, 101_i64, 100_i64] {
        book.submit(matching_engine::NewOrder {
            side: matching_engine::Side::Buy,
            price,
            quantity: 1_u64,
        })?;
    }

    let sell = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 99_i64,
        quantity: 3_u64,
    })?;

    let prices: Vec<_> = sell.trades.iter().map(|trade| trade.price).collect();
    assert_eq!(prices, [101_i64, 100_i64, 99_i64]);
    assert_eq!(sell.remaining_quantity, 0_u64);
    assert!(!sell.resting);
    assert_eq!(book.best_bid(), None);
    Ok(())
}

#[test]
fn snapshot_rejects_quantity_overflow() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    for quantity in [u64::MAX, 1_u64] {
        book.submit(matching_engine::NewOrder {
            side: matching_engine::Side::Buy,
            price: 100_i64,
            quantity,
        })?;
    }

    let error = book.snapshot().expect_err("level quantity must not wrap");
    assert!(error.to_string().contains("quantity overflow"));
    Ok(())
}
