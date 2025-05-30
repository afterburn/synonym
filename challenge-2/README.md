# Bitcoin Address Generator (Rust → WASM)

A minimal Bitcoin address generator compiled to WebAssembly using Rust. Generates BIP39 mnemonic seed phrases and derives Bitcoin P2PKH addresses.

## Features

- **Generate** new Bitcoin wallets with 12-word BIP39 mnemonics
- **Restore** wallets from existing seed phrases
- **WebAssembly** compilation for browser compatibility
- **Standards compliant** - follows BIP39 and Bitcoin address derivation

## Building

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

### Build to WASM

```bash
# Build the WASM package
./build.sh

# Serve the HTML file (use any local server)
python3 -m http.server 8000
# or
npx serve .
```

## Usage

### Web Interface

Open `index.html` in a browser after building. The interface provides:

- **Generate New Wallet**: Creates a fresh Bitcoin wallet
- **Restore from Seed**: Recovers wallet from 12-word mnemonic

### Programmatic Usage

```javascript
import init, {
  generate_bitcoin_address,
  restore_from_mnemonic,
} from "./pkg/bitcoin_wasm.js";

await init();

// Generate new wallet
const wallet = generate_bitcoin_address();
console.log(wallet.mnemonic); // 12-word seed phrase
console.log(wallet.address); // Bitcoin address (1...)
console.log(wallet.private_key); // Hex private key
console.log(wallet.public_key); // Compressed public key

// Restore from mnemonic
try {
  const restored = restore_from_mnemonic("word1 word2 ... word12");
  console.log(restored.address);
} catch (error) {
  console.error("Invalid mnemonic:", error);
}
```

## Technical Details

### Address Derivation

1. Generate 128-bit entropy → 12-word BIP39 mnemonic
2. Derive 512-bit seed using PBKDF2 with "mnemonic" salt
3. Use first 32 bytes as secp256k1 private key
4. Generate compressed public key (33 bytes)
5. Hash public key: SHA256 → RIPEMD160
6. Add version byte (0x00) + checksum (4 bytes)
7. Base58 encode → Bitcoin address

### Dependencies

- `k256` - secp256k1 elliptic curve operations
- `bip39` - mnemonic generation and seed derivation
- `sha2` - SHA256 hashing
- `ripemd` - RIPEMD160 hashing
- `bs58` - Base58 encoding
- `wasm-bindgen` - Rust ↔ JavaScript interop

### Security Notes

⚠️ **For educational purposes only**

- Private keys generated in browser memory
- No secure key storage
- Use proper hardware wallets for real funds
- Never share private keys or mnemonics

## Testing

```bash
# Run unit tests
cargo test

# Test WASM build
wasm-pack test --headless --firefox
```

## File Structure

```
├── Cargo.toml          # Rust dependencies
├── src/
│   ├── lib.rs          # Main Bitcoin address generation logic
│   └── tests.rs        # Unit tests
├── pkg/                # Generated WASM output (after build)
├── index.html          # Web interface
└── README.md           # This file
```

## Example Output

```
Mnemonic: abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
Address: 1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2
Private Key: c28a9f80738f770d527803a566cf6fc3edf6cea586c4fc4a5223a5ad797e1ac3
Public Key: 0203ad1d87c7beaab4d6d6ee80c6f01e6b85b9d2b1fa2e3a5e73d6e6b8c2e3a5c8
```

## License

MIT License - see LICENSE file for details.
