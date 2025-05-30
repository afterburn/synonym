use bip39::{Language, Mnemonic};
use bs58;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
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

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::Mnemonic;
    use std::str::FromStr;

    #[test]
    fn test_generate_bitcoin_address() {
        let address = generate_bitcoin_address();

        // Test mnemonic format
        assert!(!address.mnemonic.is_empty());
        let words: Vec<&str> = address.mnemonic.split_whitespace().collect();
        assert_eq!(words.len(), 12, "Mnemonic should have 12 words");

        // Test private key format (64 hex chars)
        assert_eq!(address.private_key.len(), 64);
        assert!(address.private_key.chars().all(|c| c.is_ascii_hexdigit()));

        // Test public key format (66 hex chars, compressed)
        assert_eq!(address.public_key.len(), 66);
        assert!(address.public_key.starts_with("02") || address.public_key.starts_with("03"));
        assert!(address.public_key.chars().all(|c| c.is_ascii_hexdigit()));

        // Test Bitcoin address format
        assert!(address.address.starts_with('1'));
        assert!(address.address.len() >= 26 && address.address.len() <= 35);

        // Test Base58 characters only
        let base58_chars = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        assert!(address.address.chars().all(|c| base58_chars.contains(c)));
    }

    #[test]
    fn test_restore_from_mnemonic_valid() {
        // Test with a known valid mnemonic
        let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let result = restore_from_mnemonic(test_mnemonic);
        assert!(result.is_ok());

        let address = result.unwrap();
        assert_eq!(address.mnemonic, test_mnemonic);
        assert!(address.address.starts_with('1'));
        assert_eq!(address.private_key.len(), 64);
        assert_eq!(address.public_key.len(), 66);
    }

    #[test]
    fn test_restore_from_mnemonic_invalid() {
        // Test with invalid mnemonic
        let invalid_mnemonic = "invalid mnemonic phrase that should fail";
        let result = restore_from_mnemonic(invalid_mnemonic);
        assert!(result.is_err());

        // Test with empty string
        let result = restore_from_mnemonic("");
        assert!(result.is_err());

        // Test with wrong word count
        let result = restore_from_mnemonic("abandon abandon abandon");
        assert!(result.is_err());
    }

    #[test]
    fn test_deterministic_generation() {
        // Same mnemonic should always produce same results
        let test_mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let result1 = restore_from_mnemonic(test_mnemonic).unwrap();
        let result2 = restore_from_mnemonic(test_mnemonic).unwrap();

        assert_eq!(result1.address, result2.address);
        assert_eq!(result1.private_key, result2.private_key);
        assert_eq!(result1.public_key, result2.public_key);
    }

    #[test]
    fn test_roundtrip_generate_restore() {
        // Generate an address, then restore it from mnemonic
        let original = generate_bitcoin_address();
        let restored = restore_from_mnemonic(&original.mnemonic).unwrap();

        assert_eq!(original.address, restored.address);
        assert_eq!(original.private_key, restored.private_key);
        assert_eq!(original.public_key, restored.public_key);
        assert_eq!(original.mnemonic, restored.mnemonic);
    }

    #[test]
    fn test_mnemonic_validation() {
        // Test that generated mnemonics are valid BIP39
        for _ in 0..5 {
            let address = generate_bitcoin_address();
            let mnemonic = Mnemonic::from_str(&address.mnemonic);
            assert!(mnemonic.is_ok(), "Generated mnemonic should be valid BIP39");
        }
    }

    #[test]
    fn test_private_key_range() {
        // Test that private keys are within valid secp256k1 range
        let address = generate_bitcoin_address();
        let private_key_bytes = hex::decode(&address.private_key).unwrap();

        // Should be 32 bytes
        assert_eq!(private_key_bytes.len(), 32);

        // Should not be zero
        assert!(private_key_bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn test_unique_generation() {
        // Generate multiple addresses and ensure they're unique
        let mut addresses = std::collections::HashSet::new();
        let mut mnemonics = std::collections::HashSet::new();

        for _ in 0..10 {
            let wallet = generate_bitcoin_address();

            // Each generation should be unique
            assert!(addresses.insert(wallet.address.clone()));
            assert!(mnemonics.insert(wallet.mnemonic.clone()));
        }
    }

    #[test]
    fn test_address_checksum() {
        // Generate address and verify it has valid checksum structure
        let wallet = generate_bitcoin_address();

        // Decode Base58 address
        let decoded = bs58::decode(&wallet.address).into_vec().unwrap();

        // Should be 25 bytes total (1 version + 20 hash + 4 checksum)
        assert_eq!(decoded.len(), 25);

        // First byte should be 0x00 (mainnet P2PKH)
        assert_eq!(decoded[0], 0x00);

        // Verify checksum
        let payload = &decoded[0..21];
        let checksum = &decoded[21..25];

        let hash1 = Sha256::digest(payload);
        let hash2 = Sha256::digest(&hash1);
        let expected_checksum = &hash2[0..4];

        assert_eq!(checksum, expected_checksum);
    }
}
