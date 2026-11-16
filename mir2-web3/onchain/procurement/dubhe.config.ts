import { defineConfig } from '@0xobelisk/sui-common';

// Schema contract for the real Dubhe ObjectStorage fields. Gold and equipment stay in the game DB.
export const dubheConfig = defineConfig({
  name: 'mir2_procurement',
  description: 'Server-custodied personal equipment procurement; not a trustless asset exchange',
  objects: {
    order: {
      fields: {
        publish_intent_hash: 'vector<u8>', buyer_ref: 'vector<u8>', buyer_public_key: 'vector<u8>',
        template_id: 'u32', conditions_hash: 'vector<u8>', conditions_bcs: 'vector<u8>', quantity: 'u64', price_gold: 'u64',
        expires_at_ms: 'u64', rule_version: 'u64', escrow_lock_id: 'vector<u8>',
        status: 'u8', fence: 'u64', fill_id: 'vector<u8>', seller_ref: 'vector<u8>',
        asset_instance_hash: 'vector<u8>'
      }
    }
  }
});
