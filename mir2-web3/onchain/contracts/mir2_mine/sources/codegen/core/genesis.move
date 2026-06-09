#[allow(lint(share_owned))]module mir2_mine::mir2_mine_genesis {

  use std::ascii::string;

  use sui::clock::Clock;

  use mir2_mine::mir2_mine_dapp_system;

  public entry fun run(clock: &Clock, ctx: &mut TxContext) {
    // Create schemas
    let mut schema = mir2_mine::mir2_mine_schema::create(ctx);
    // Setup default storage
    mir2_mine_dapp_system::create(&mut schema, string(b"mir2_mine"),string(b"MIR2 on-chain smart mine"), clock , ctx);
    // Logic that needs to be automated once the contract is deployed
    mir2_mine::mir2_mine_deploy_hook::run(&mut schema,  ctx);
    // Authorize schemas and public share objects
    sui::transfer::public_share_object(schema);
  }
}
