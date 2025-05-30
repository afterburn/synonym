use bip39::{Language, Mnemonic};
use bs58;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::{elliptic_curve::rand_core::OsRng, PublicKey, SecretKey};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub struct BitcoinAddress {
    mnemonic: String,
    private_key: String,
    public_key: String,
    address: String,
}

#[wasm_bindgen]
impl BitcoinAddress {
    #[wasm_bindgen(getter)]
    pub fn mnemonic(&self) -> String {
        self.mnemonic.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn private_key(&self) -> String {
        self.private_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn public_key(&self) -> String {
        self.public_key.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn address(&self) -> String {
        self.address.clone()
    }
}

#[wasm_bindgen]
pub fn generate_bitcoin_address() -> BitcoinAddress {
    // Generate 12-word mnemonic
    let mnemonic = Mnemonic::generate_in(Language::English, 12).unwrap();
    let mnemonic_phrase = mnemonic.to_string();

    // Derive seed from mnemonic (no passphrase)
    let seed = mnemonic.to_seed("");

    // Use first 32 bytes as private key
    let private_key_bytes = &seed[0..32];
    let secret_key = SecretKey::from_slice(private_key_bytes).unwrap();
    let private_key_hex = hex::encode(secret_key.to_bytes());

    // Derive public key
    let public_key = secret_key.public_key();
    let public_key_bytes = public_key.to_encoded_point(true).as_bytes().to_vec();
    let public_key_hex = hex::encode(&public_key_bytes);

    // Hash public key (SHA256 then RIPEMD160)
    let sha256_hash = Sha256::digest(&public_key_bytes);
    let ripemd160_hash = Ripemd160::digest(&sha256_hash);

    // Add version byte (0x00 for mainnet)
    let mut versioned_hash = vec![0x00];
    versioned_hash.extend_from_slice(&ripemd160_hash);

    // Calculate checksum (double SHA256)
    let checksum_hash = Sha256::digest(&Sha256::digest(&versioned_hash));
    let checksum = &checksum_hash[0..4];

    // Append checksum
    let mut final_bytes = versioned_hash;
    final_bytes.extend_from_slice(checksum);

    // Base58 encode
    let address = bs58::encode(&final_bytes).into_string();

    BitcoinAddress {
        mnemonic: mnemonic_phrase,
        private_key: private_key_hex,
        public_key: public_key_hex,
        address,
    }
}

#[wasm_bindgen]
pub fn restore_from_mnemonic(mnemonic_phrase: &str) -> Result<BitcoinAddress, String> {
    // Parse the mnemonic
    let mnemonic = match Mnemonic::parse(mnemonic_phrase) {
        Ok(m) => m,
        Err(e) => return Err(format!("Invalid mnemonic: {}", e)),
    };

    // Derive seed from mnemonic (no passphrase)
    let seed = mnemonic.to_seed("");

    // Use first 32 bytes as private key
    let private_key_bytes = &seed[0..32];
    let secret_key = match SecretKey::from_slice(private_key_bytes) {
        Ok(sk) => sk,
        Err(e) => return Err(format!("Invalid private key: {}", e)),
    };
    let private_key_hex = hex::encode(secret_key.to_bytes());

    // Derive public key
    let public_key = secret_key.public_key();
    let public_key_bytes = public_key.to_encoded_point(true).as_bytes().to_vec();
    let public_key_hex = hex::encode(&public_key_bytes);

    // Hash public key (SHA256 then RIPEMD160)
    let sha256_hash = Sha256::digest(&public_key_bytes);
    let ripemd160_hash = Ripemd160::digest(&sha256_hash);

    // Add version byte (0x00 for mainnet)
    let mut versioned_hash = vec![0x00];
    versioned_hash.extend_from_slice(&ripemd160_hash);

    // Calculate checksum (double SHA256)
    let checksum_hash = Sha256::digest(&Sha256::digest(&versioned_hash));
    let checksum = &checksum_hash[0..4];

    // Append checksum
    let mut final_bytes = versioned_hash;
    final_bytes.extend_from_slice(checksum);

    // Base58 encode
    let address = bs58::encode(&final_bytes).into_string();

    Ok(BitcoinAddress {
        mnemonic: mnemonic_phrase.to_string(),
        private_key: private_key_hex,
        public_key: public_key_hex,
        address,
    })
}
