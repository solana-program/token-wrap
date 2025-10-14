import {
  address,
  appendTransactionMessageInstructions,
  assertIsSendableTransaction,
  assertIsTransactionWithBlockhashLifetime,
  createKeyPairSignerFromBytes,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createTransactionMessage,
  getSignatureFromTransaction,
  pipe,
  sendAndConfirmTransactionFactory,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import { getAddressEncoder, getProgramDerivedAddress, getUtf8Encoder } from '@solana/kit';
import {
  findWrappedMintAuthorityPda,
  findWrappedMintPda,
  getSyncMetadataToToken2022Instruction,
} from '../index';

// =================================================================
// PREREQUISITES:
// =================================================================
// 1. An unwrapped SPL Token mint with Metaplex metadata must exist.
// 2. The corresponding wrapped Token-2022 mint for it must have been created
//    via the `create-mint` command or `createMint` helper.
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

// Source Mint: An existing SPL Token mint with Metaplex metadata
const UNWRAPPED_SPL_TOKEN_MINT = address('8owJWKMiKfMKYbPmobyZAwXibNFcY7Roj6quktaeqxGL');

async function main() {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PRIVATE_KEY_PAIR);
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  console.log('======== Syncing: SPL Token -> Token-2022 ========');

  // To sync from an SPL Token mint, the client must resolve and provide the
  // Metaplex Metadata PDA as the `sourceMetadata` account.
  // Derive it using the known Metadata program and seeds: ['metadata', programId, mint]
  const TOKEN_METADATA_PROGRAM_ADDRESS = address('metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s');
  const [metaplexMetadataPda] = await getProgramDerivedAddress({
    programAddress: TOKEN_METADATA_PROGRAM_ADDRESS,
    seeds: [
      getUtf8Encoder().encode('metadata'),
      getAddressEncoder().encode(TOKEN_METADATA_PROGRAM_ADDRESS),
      getAddressEncoder().encode(UNWRAPPED_SPL_TOKEN_MINT),
    ],
  });
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_SPL_TOKEN_MINT,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({
    wrappedMint,
  });

  const syncToT22Ix = getSyncMetadataToToken2022Instruction({
    wrappedMint,
    wrappedMintAuthority,
    unwrappedMint: UNWRAPPED_SPL_TOKEN_MINT,
    // When the source mint is a standard SPL Token, `sourceMetadata` MUST be
    // the address of its Metaplex Metadata PDA.
    sourceMetadata: metaplexMetadataPda,
  });

  const transaction = await pipe(
    createTransactionMessage({ version: 0 }),
    tx => setTransactionMessageFeePayerSigner(payer, tx),
    tx => setTransactionMessageLifetimeUsingBlockhash(blockhash, tx),
    tx => appendTransactionMessageInstructions([syncToT22Ix], tx),
    tx => signTransactionMessageWithSigners(tx),
  );
  assertIsSendableTransaction(transaction);
  assertIsTransactionWithBlockhashLifetime(transaction);
  const signature = getSignatureFromTransaction(transaction);
  await sendAndConfirm(transaction, { commitment: 'confirmed' });

  console.log('Successfully synced metadata to Token-2022 mint.');
  console.log('Signature:', signature);
}

void main();
