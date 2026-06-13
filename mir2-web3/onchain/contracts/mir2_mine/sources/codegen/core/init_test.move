#[test_only]module mir2_mine::mir2_mine_init_test {

  use sui::clock;

  use sui::test_scenario;

  use sui::test_scenario::Scenario;

  public fun deploy_dapp_for_testing(scenario: &mut Scenario) {
    let ctx = test_scenario::ctx(scenario);
    let clock = clock::create_for_testing(ctx);
    mir2_mine::mir2_mine_genesis::run(&clock,  ctx);
    clock::destroy_for_testing(clock);
    test_scenario::next_tx(scenario, ctx.sender());
  }
}
