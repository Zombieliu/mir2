fn main() {
    let iterations = std::env::var("GATE10_ACCEPTANCE_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1);
    if !(1..=10_000).contains(&iterations) {
        eprintln!("GATE10_ACCEPTANCE_ITERATIONS must be in 1..=10000");
        std::process::exit(2);
    }
    let mut final_evidence = None;
    let mut expected_batch = None;
    for iteration in 1..=iterations {
        let evidence = match mir2_gateway::run_gate10_acceptance() {
            Ok(evidence) => evidence,
            Err(error) => {
                eprintln!("Gate 10 acceptance iteration {iteration} failed: {error}");
                std::process::exit(1);
            }
        };
        let fingerprint = (
            evidence.reward_batch.batch_id.clone(),
            evidence.reward_batch.merkle_root.clone(),
            evidence.reward_batch.total_reward,
        );
        if let Some(expected) = expected_batch.as_ref() {
            if expected != &fingerprint {
                eprintln!("Gate 10 acceptance iteration {iteration} was nondeterministic");
                std::process::exit(1);
            }
        } else {
            expected_batch = Some(fingerprint);
        }
        final_evidence = Some(evidence);
    }
    let evidence = final_evidence.expect("positive iteration count checked above");
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).expect("Gate 10 evidence must serialize")
    );
    print!("{}", evidence.report.prometheus());
    println!("# TYPE obelisk_beta_acceptance_iterations counter");
    println!("obelisk_beta_acceptance_iterations {iterations}");
}
