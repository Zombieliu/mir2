module mir2_mine::mir2_mine_dapp_system {

  use std::ascii::String;

  use std::ascii;

  use dubhe::type_info;

  use sui::clock::Clock;

  use sui::transfer::public_share_object;

  use mir2_mine::mir2_mine_schema::Schema;

  use mir2_mine::mir2_mine_dapp_metadata;

  use mir2_mine::mir2_mine_dapp_metadata::DappMetadata;

  use dubhe::storage::add_field;

  use dubhe::storage_value;

  use dubhe::storage_value::StorageValue;

  public entry fun set_metadata(
    schema: &mut Schema,
    name: String,
    description: String,
    cover_url: vector<String>,
    website_url: String,
    partners: vector<String>,
    ctx: &TxContext,
  ) {
    let admin = schema.dapp__admin().try_get();
    assert!(admin == option::some(ctx.sender()), 0);
    let created_at = schema.dapp__metadata().get().get_created_at();
    schema.dapp__metadata().set(
            mir2_mine_dapp_metadata::new(
                name,
                description,
                cover_url,
                website_url,
                created_at,
                partners
            )
        );
  }

  public entry fun transfer_ownership(schema: &mut Schema, new_admin: address, ctx: &mut TxContext) {
    let admin = schema.dapp__admin().try_get();
    assert!(admin == option::some(ctx.sender()), 0);
    schema.dapp__admin().set(new_admin);
  }

  public entry fun set_safe_mode(schema: &mut Schema, safe_mode: bool, ctx: &TxContext) {
    let admin = schema.dapp__admin().try_get();
    assert!(admin == option::some(ctx.sender()), 0);
    schema.dapp__safe_mode().set(safe_mode);
  }

  public fun ensure_no_safe_mode(schema: &mut Schema) {
    assert!(!schema.dapp__safe_mode()[], 0);
  }

  public fun ensure_has_authority(schema: &mut Schema, ctx: &TxContext) {
    assert!(schema.dapp__admin().get() == ctx.sender(), 0);
  }

  public fun ensure_has_schema<S: key + store>(schema: &mut Schema, new_schema: &S) {
    let schema_id = object::id_address(new_schema);
    assert!(schema.dapp__authorised_schemas().get().contains(&schema_id), 0);
  }

  public(package) fun create(
    schema: &mut Schema,
    name: String,
    description: String,
    clock: &Clock,
    ctx: &mut TxContext,
  ) {
    add_field<StorageValue<address>>(schema.id(), b"dapp__admin", storage_value::new(b"dapp__admin", ctx));
    add_field<StorageValue<address>>(schema.id(), b"dapp__package_id", storage_value::new(b"dapp__package_id", ctx));
    add_field<StorageValue<u32>>(schema.id(), b"dapp__version", storage_value::new(b"dapp__version", ctx));
    add_field<StorageValue<DappMetadata>>(schema.id(), b"dapp__metadata", storage_value::new(b"dapp__metadata", ctx));
    add_field<StorageValue<bool>>(schema.id(), b"dapp__safe_mode", storage_value::new(b"dapp__safe_mode", ctx));
    add_field<StorageValue<vector<address>>>(schema.id(), b"dapp__authorised_schemas", storage_value::new(b"dapp__authorised_schemas", ctx));
    schema.dapp__metadata().set(
            mir2_mine_dapp_metadata::new(
                name,
                description,
                vector[],
                ascii::string(b""),
                clock.timestamp_ms(),
                vector[]
            )
        );
    let package_id = type_info::current_package_id<Schema>();
    schema.dapp__package_id().set(package_id);
    schema.dapp__admin().set(ctx.sender());
    schema.dapp__version().set(1);
    schema.dapp__safe_mode().set(false);
    schema.dapp__authorised_schemas().set(vector[]);
  }

  public(package) fun add_schema<S: key + store>(schema: &mut Schema, new_schema: S) {
    let mut schemas = schema.dapp__authorised_schemas()[];
    schemas.push_back(object::id_address<S>(&new_schema));
    schema.dapp__authorised_schemas().set(schemas);
    public_share_object(new_schema);
  }
}
