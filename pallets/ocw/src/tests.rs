use super::*;
use crate::mock::{Call, *};
use frame_support::assert_ok;
use sp_core::{offchain::testing};
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use sp_runtime::{
	offchain::{OffchainDbExt, OffchainWorkerExt, TransactionPoolExt},
	Permill, RuntimeAppPublic,
};
use sp_std::convert::{From};
use std::sync::Arc;

#[test]
fn parse_price_unit_test() {
	let json_data = r#"{"data": {"priceUsd": "27.0080710784431881"}}"#;
	let price = Ocw::parse_price(json_data).expect("parse failed");
	// println!("price: {:?}", price);
	assert_eq!(price.0, 27);
	assert_eq!(price.1, Permill::from_parts(8071));

	let json_data = r#"{"data": {"priceUsd": "27.2380710784431881"}}"#;
	let price = Ocw::parse_price(json_data).expect("parse failed");
	assert_eq!(price.0, 27);
	assert_eq!(price.1, Permill::from_parts(238071));

	let json_data = r#"{"data": {"priceUsd": "0.2380"}}"#;
	let price = Ocw::parse_price(json_data).expect("parse failed");
	assert_eq!(price.0, 0);
	assert_eq!(price.1, Permill::from_parts(2380));

	let json_data = r#"{"data": {"priceUsd": "1"}}"#;
	let price = Ocw::parse_price(json_data).expect("parse failed");
	assert_eq!(price.0, 1);
	assert_eq!(price.1, Permill::from_parts(0));
}

fn price_oracle_response(state: &mut testing::OffchainState) {
	state.expect_request(testing::PendingRequest {
		method: "GET".into(),
		uri: HTTP_REMOTE_DOT_PRICE.into(),
		response: Some(
			br#"
        {
            "data": {
                "id": "polkadot",
                "rank": "10",
                "symbol": "DOT",
                "name": "Polkadot",
                "supply": "1074340278.6254400000000000",
                "maxSupply": null,
                "marketCapUsd": "30350117581.7208334961700130",
                "volumeUsd24Hr": "803942185.4206424031495485",
                "priceUsd": "28.2500043845997839",
                "changePercent24Hr": "2.4606795772541142",
                "vwap24Hr": "27.9288013090470396",
                "explorer": "https://polkascan.io/polkadot"
            },
            "timestamp": 1642328751709
        }
        
        "#
			.to_vec(),
		),
		sent: true,
		..Default::default()
	});
}

#[test]
fn should_submit_unsigned_transaction_on_chain_for_any_account() {
	let mut ext = new_test_ext();
	let (offchain, state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();

	let keystore = KeyStore::new();

	SyncCryptoStore::sr25519_generate_new(&keystore, crate::crypto::Public::ID, Some(&"//Alice"))
		.unwrap();

	let public_key = SyncCryptoStore::sr25519_public_keys(&keystore, crate::crypto::Public::ID)
		.get(0)
		.unwrap()
		.clone();

	// let account = MultiSigner::Sr25519(public_key.clone()).into_account();
	// println!("account: {:?}", account);

	ext.register_extension(OffchainDbExt::new(offchain.clone()));
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(KeystoreExt(Arc::new(keystore)));

	price_oracle_response(&mut state.write());

	let price_payload = PricePayload {
		price: (28, Permill::from_parts(250004)),
		public: <Test as SigningTypes>::Public::from(public_key),
	};

	// let signature = price_payload.sign::<crypto::TestAuthId>().unwrap();
	ext.execute_with(|| {
        // set ALCIE is price provider
        assert_ok!(Ocw::set_price_provider(Origin::root(), ALICE));
		// when
		assert_ok!(Ocw::fetch_price_info());
		// then
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature, None);
		if let Call::Ocw(crate::Call::submit_price_unsigned_with_signed_payload(body, signature)) =
			tx.call
		{
			assert_eq!(body, price_payload);

			let signature_valid = <PricePayload<<Test as SigningTypes>::Public> as SignedPayload<
				Test,
			>>::verify::<crypto::TestAuthId>(&price_payload, signature);

			assert!(signature_valid);
		}
	});
}
