# On chain exchange

A small standalone limit-order matching engine written entirely in safe Rust.
It requires no database, server, container, or other runtime service.

## Matching algorithm and justification

The engine uses price-time priority:

1. The best available price matches first.
2. At the same price, the oldest resting order matches first (FIFO).
3. The resting maker's price is the execution price.
4. Partial fills are supported; unfilled quantity remains on the book.

Price-time priority was selected because it is deterministic, easy to audit, and
is the conventional fairness rule for a continuous limit-order book.

## In-memory data structures

- `BTreeMap<i64, VecDeque<Order>>` stores ordered price levels.
- `VecDeque` preserves FIFO priority within a price level.
- `HashMap<OrderId, Location>` locates active orders for cancellation.
- `HashSet<OrderId>` prevents reuse of an order ID after fill or cancellation.

One `OrderBook` represents one instrument and has one mutable owner. Production
systems can scale by assigning instruments to independent matcher shards.

Cancellation scans only the selected price level. For a production engine with
very large levels, an arena-backed intrusive linked list can make cancellation
strictly O(1), but the simpler representation is easier to review for this test.

## Run

```bash
cargo test
cargo run
```

The executable submits a few orders, prints the resulting trades and book, then
cancels the remaining order.

## Complexity

Let `P` be the number of active price levels and `L` the number of orders at a
selected price level.

- Best bid/ask lookup: `O(log P)` through the ordered map API.
- Add a new price level: `O(log P)`; append at an existing level: `O(1)`.
- Matching one maker order: `O(1)` after locating the best level.
- Cancellation: `O(log P + L)` in this intentionally simple implementation.
