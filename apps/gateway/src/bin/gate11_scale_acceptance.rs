use mir2_gateway::run_gate11_scale_acceptance;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence = run_gate11_scale_acceptance()?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
