use mir2_gateway::{run_gate11_acceptance, run_gate11_scale_acceptance};

#[test]
fn real_mir2_map_workload_survives_remote_zone_handoff_and_failover() {
    let evidence = run_gate11_acceptance().expect("Gate 11.1 acceptance should pass");

    evidence
        .require_accepted()
        .expect("Gate 11.1 evidence should remain accepted");
}

#[test]
fn multi_session_multi_map_workload_survives_two_fenced_failovers() {
    let evidence = run_gate11_scale_acceptance().expect("Gate 11.3 scale acceptance should pass");

    evidence
        .require_accepted()
        .expect("Gate 11.3 evidence should remain accepted");
}
