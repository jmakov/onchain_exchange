#![forbid(unsafe_code)]

fn main() -> eyre::Result<()> {
    let mut book = matching_engine::OrderBook::new();

    let first_sell = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 60_000_i64,
        quantity: 2_u64,
    })?;

    let second_sell = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Sell,
        price: 60_000_i64,
        quantity: 3_u64,
    })?;

    // Price-time priority fills the first resting sell before the second.
    let buy = book.submit(matching_engine::NewOrder {
        side: matching_engine::Side::Buy,
        price: 61_000_i64,
        quantity: 4_u64,
    })?;

    println!("first sell order ID: {}", first_sell.order_id);
    println!("second sell order ID: {}", second_sell.order_id);
    println!("buy result:\n{buy:#?}");
    println!("remaining book:\n{:#?}", book.snapshot()?);

    Ok(())
}
