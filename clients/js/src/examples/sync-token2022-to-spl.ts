import {
  address,
  appendTransactionMessageInstructions,
  assertIsSendableTransaction,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getAddressEncoder,
  getProgramDerivedAddress,
  getSignatureFromTransaction,
  getUtf8Encoder,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_PROGRAM_ADDRESS } from '@solana-program/token';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getSyncMetadataToSplTokenInstruction,
} from '../index';

// =================================================================
// PREREQUISITES:
// =================================================================
// 1. An unwrapped Token-2022 mint with `TokenMetadata` and `MetadataPointer`
//    extensions must exist. The pointer should point to the mint itself.
// 2. The corresponding wrapped SPL Token mint for it must have been created
//    via the `create-mint` command or `createMint` helper.
// 3. The wrapped mint authority PDA must be funded with enough SOL to pay for
//    the creation of the Metaplex metadata account, as it acts as the payer
//    for the CPI to the Metaplex program.
//
// This example ASSUMES these accounts already exist and focuses only on the
// transaction to sync the metadata.

// Replace these consts with the addresses from your setup
const PRIVATE_KEY_PAIR = new Uint8Array([
  242, 30, 38, 177, 152, 71, 235, 193, 93, 30, 119, 131, 42, 186, 202, 7, 45, 250, 126, 135, 107,
  137, 38, 91, 202, 212, 12, 8, 154, 213, 163, 200, 23, 237, 17, 163, 3, 135, 34, 126, 235, 146,
  251, 18, 199, 101, 153, 249, 134, 88, 219, 68, 167, 136, 234, 195, 12, 34, 184, 85, 234, 25, 125,
  94,
]);

// Source Mint: An existing Token-2022 mint with metadata extensions
const UNWRAPPED_TOKEN_2022_MINT = address('5xte8yNSUTrTtfdptekeA4QJyo8zZdanpDJojrRaXP1Y');

async function main() {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  console.log('======== Syncing: Token-2022 -> SPL Token ========');

  // Derive the wrapped SPL Token mint PDA
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_TOKEN_2022_MINT,
    wrappedTokenProgram: TOKEN_PROGRAM_ADDRESS,
  });

  // Derive the Metaplex Metadata PDA for the wrapped mint.
  const TOKEN_METADATA_PROGRAM_ADDRESS = address('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');
  const [metaplexMetadataPda] = await getProgramDerivedAddress({
    programAddress: TOKEN_METADATA_PROGRAM_ADDRESS,
    seeds: [
      getUtf8Encoder().encode('metadata'),
      getAddressEncoder().encode(TOKEN_METADATA_PROGRAM_ADDRESS),
      getAddressEncoder().encode(wrappedMint),
    ],
  });

  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({
    wrappedMint,
  });

  const syncToSplIx = getSyncMetadataToSplTokenInstruction({
    metaplexMetadata: metaplexMetadataPda,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedMint: UNWRAPPED_TOKEN_2022_MINT,
  });

  const transaction = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([syncToSplIx], tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(transaction);
  const signature = getSignatureFromTransaction(transaction);
  await sendAndConfirm(transaction, { commitment: 'confirmed' });

  console.log('Successfully synced metadata to SPL Token Metaplex account.');
  console.log('Signature:', signature);
}

void main();
