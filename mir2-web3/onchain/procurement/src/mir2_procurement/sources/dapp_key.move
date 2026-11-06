// SPDX-License-Identifier: Apache-2.0
module mir2_procurement::dapp_key;

public struct DappKey has copy, drop {}

/// Only this package can construct the Dubhe write authorization token.
public(package) fun new(): DappKey { DappKey {} }

public fun package_id(): address {
    sui::address::from_ascii_bytes(std::type_name::with_defining_ids<DappKey>().address_string().as_bytes())
}
