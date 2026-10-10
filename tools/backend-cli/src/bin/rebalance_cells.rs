use gaming_cafe_api::moving::rebalance::{self, Measurements};
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Rebalancing failed: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut input = None;
    let mut drain = None;
    let mut maximum = 10;
    let mut apply = false;
    let mut offline = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("rebalance_cells --measurements FILE [--drain CELL_UUID] [--max-moves 1..100] [--apply]\nrebalance_cells --decommission CELL_UUID\nPreview by default. Uses CONTROL_DATABASE_URL and benchmark-backed cells.capacity_weights. --apply atomically reserves target capacity and enqueues moves; --drain also marks the source DRAINING.");
                return Ok(());
            }
            "--measurements" => input = Some(args.next().ok_or("Missing measurements file")?),
            "--drain" => drain = Some(Uuid::parse_str(&args.next().ok_or("Missing drain cell")?)?),
            "--max-moves" => maximum = args.next().ok_or("Missing maximum")?.parse()?,
            "--apply" => apply = true,
            "--decommission" => {
                offline = Some(Uuid::parse_str(&args.next().ok_or("Missing cell UUID")?)?)
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if let Some(cell) = offline {
        if input.is_some() || drain.is_some() || apply {
            return Err("Use --decommission CELL_UUID by itself".into());
        }
        gaming_cafe_api::config::load_dotenv();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&std::env::var("CONTROL_DATABASE_URL")?)
            .await?;
        rebalance::decommission(&pool, cell).await?;
        println!("Cell {cell} is OFFLINE");
        return Ok(());
    }
    let measurements: Measurements =
        serde_json::from_slice(&std::fs::read(input.ok_or("Pass --measurements FILE")?)?)?;
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let plan = rebalance::run(&pool, &measurements, drain, maximum, apply).await?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}
