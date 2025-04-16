import {
  address,
  createKeyPairSignerFromBytes,
  createNoopSigner,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getBase58Decoder,
  getSignatureFromTransaction,
  partiallySignTransactionMessageWithSigners,
  sendAndConfirmTransactionFactory,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, findWrappedMintPda } from '../generated';
import { combinedMultisigWrapTx, multisigOfflineSignWrapTx } from '../wrap';
import { createEscrowAccountTx, createTokenAccountTx, getOwnerFromAccount } from '../utilities';
import { createMintTx } from '../create-mint';

// Replace these consts with your own
const PAYER_KEYPAIR_BYTES = new Uint8Array([
  242, 30, 38, 177, 152, 71, 235, 193, 93, 30, 119, 131, 42, 186, 202, 7, 45, 250, 126, 135, 107,
  137, 38, 91, 202, 212, 12, 8, 154, 213, 163, 200, 23, 237, 17, 163, 3, 135, 34, 126, 235, 146,
  251, 18, 199, 101, 153, 249, 134, 88, 219, 68, 167, 136, 234, 195, 12, 34, 184, 85, 234, 25, 125,
  94,
]);

// Create using CLI: spl-token create-multisig 2 $SIGNER_1_PUBKEY $SIGNER_2_PUBKEY
const MULTISIG_PUBKEY = address('5VzZUs4UzeCS1AqUSc6Eq6HDQzx4UNAA6YUhXphNwxpi');
const SIGNER_A_KEYPAIR_BYTES = new Uint8Array([
  37, 105, 121, 65, 52, 25, 194, 10, 250, 52, 209, 193, 144, 236, 52, 118, 114, 72, 86, 211, 29, 55,
  84, 48, 183, 127, 187, 146, 4, 224, 124, 20, 72, 247, 148, 73, 5, 240, 41, 35, 215, 49, 198, 9,
  81, 97, 179, 35, 234, 149, 145, 8, 116, 71, 248, 156, 20, 121, 96, 76, 248, 232, 211, 29,
]);
const SIGNER_B_KEYPAIR_BYTES = new Uint8Array([
  84, 253, 143, 134, 166, 119, 48, 101, 46, 57, 185, 171, 52, 59, 195, 86, 43, 210, 34, 78, 73, 183,
  70, 133, 116, 138, 4, 155, 190, 120, 73, 159, 176, 189, 2, 65, 57, 8, 139, 59, 33, 31, 37, 190,
  22, 229, 62, 48, 57, 54, 35, 76, 231, 166, 128, 7, 49, 203, 195, 3, 20, 62, 116, 197,
]);

const UNWRAPPED_MINT_ADDRESS = address('3nnwRbmrRU1zjvFFCW63rEERyV47ztmBiKSLUowLZnb5');
const UNWRAPPED_TOKEN_ACCOUNT = address('BDXNFqkVohLkNmHQWHeskKAbCWULPXaBT9cw2UdN9Wyp'); // Must be owned by multisig account
const AMOUNT_TO_WRAP = 100n;

const main = async () => {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PAYER_KEYPAIR_BYTES);

  // Initialize the wrapped mint
  const createMintMessage = await createMintTx({
    rpc,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  const signedCreateMintTx = await signTransactionMessageWithSigners(createMintMessage.tx);
  await sendAndConfirm(signedCreateMintTx, { commitment: 'confirmed' });
  const createMintSignature = getSignatureFromTransaction(signedCreateMintTx);

  console.log('======== Create Mint Successful ========');
  console.log('Wrapped Mint:', createMintMessage.wrappedMint);
  console.log('Backpointer:', createMintMessage.backpointer);
  console.log('Funded wrapped mint lamports:', createMintMessage.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', createMintMessage.fundedBackpointerLamports);
  console.log('Signature:', createMintSignature);

  // === Setup accounts needed for wrap ===

  // Create escrow account that with hold unwrapped tokens
  const createEscrowMessage = await createEscrowAccountTx({
    rpc,
    payer,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const signedCreateEscrowTx = await signTransactionMessageWithSigners(createEscrowMessage.tx);
  await sendAndConfirm(signedCreateEscrowTx, { commitment: 'confirmed' });

  // Create recipient account where wrapped tokens will be minted to
  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const recipientTokenAccountMessage = await createTokenAccountTx({
    rpc,
    payer,
    mint: wrappedMint,
    tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    owner: payer.address,
  });
  const signedRecipientAccountTx = await signTransactionMessageWithSigners(
    recipientTokenAccountMessage.tx,
  );
  await sendAndConfirm(signedRecipientAccountTx, { commitment: 'confirmed' });

  const unwrappedTokenProgram = await getOwnerFromAccount(rpc, UNWRAPPED_TOKEN_ACCOUNT);
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });

  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  const signerA = await createKeyPairSignerFromBytes(SIGNER_A_KEYPAIR_BYTES);
  const signerB = await createKeyPairSignerFromBytes(SIGNER_B_KEYPAIR_BYTES);

  // Two signers and the payer sign the transaction independently

  const wrapTxA = await multisigOfflineSignWrapTx({
    payer: createNoopSigner(payer.address),
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount: createEscrowMessage.keyPair.address,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount: recipientTokenAccountMessage.keyPair.address,
    transferAuthority: MULTISIG_PUBKEY,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [signerA, createNoopSigner(signerB.address)],
    blockhash,
  });
  const signedWrapTxA = await partiallySignTransactionMessageWithSigners(wrapTxA);

  const wrapTxB = await multisigOfflineSignWrapTx({
    payer: createNoopSigner(payer.address),
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount: createEscrowMessage.keyPair.address,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount: recipientTokenAccountMessage.keyPair.address,
    transferAuthority: MULTISIG_PUBKEY,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), signerB],
    blockhash,
  });
  const signedWrapTxB = await partiallySignTransactionMessageWithSigners(wrapTxB);

  const wrapTxC = await multisigOfflineSignWrapTx({
    payer,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount: createEscrowMessage.keyPair.address,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount: recipientTokenAccountMessage.keyPair.address,
    transferAuthority: MULTISIG_PUBKEY,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), createNoopSigner(signerB.address)],
    blockhash,
  });
  const signedWrapTxC = await partiallySignTransactionMessageWithSigners(wrapTxC);

  // Lastly, all signatures are combined together and broadcast

  const combinedTx = await combinedMultisigWrapTx({
    signedTxs: [signedWrapTxA, signedWrapTxB, signedWrapTxC],
    blockhash,
  });
  await sendAndConfirm(combinedTx, { commitment: 'confirmed' });

  console.log('======== Confirmed Multisig Tx ✅ ========');
  for (const [pubkey, signature] of Object.entries(combinedTx.signatures)) {
    if (signature) {
      const base58Sig = getBase58Decoder().decode(signature);
      console.log(`pubkey: ${pubkey}`);
      console.log(`signature: ${base58Sig}`);
      console.log('-----');
    }
  }
};

void main();
