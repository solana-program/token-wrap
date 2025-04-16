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
const MULTISIG_PUBKEY = address('2XBevFsu4pnZpB9PewYKAJHNyx9dFQf3MaiGBszF5fm8');
const SIGNER_A_KEYPAIR_BYTES = new Uint8Array([
  210, 190, 232, 169, 113, 107, 195, 87, 14, 9, 125, 106, 41, 174, 131, 9, 29, 144, 95, 134, 68,
  123, 80, 215, 194, 30, 170, 140, 33, 175, 69, 126, 201, 176, 240, 30, 173, 145, 185, 162, 231,
  196, 71, 236, 233, 153, 42, 243, 146, 82, 70, 153, 129, 194, 156, 110, 84, 18, 71, 143, 38, 244,
  232, 58,
]);
const SIGNER_B_KEYPAIR_BYTES = new Uint8Array([
  37, 161, 191, 225, 59, 192, 226, 154, 168, 4, 189, 155, 235, 240, 187, 210, 230, 176, 133, 163, 6,
  132, 229, 129, 10, 9, 67, 88, 215, 124, 195, 243, 189, 178, 12, 18, 216, 91, 154, 193, 75, 164,
  71, 224, 106, 148, 225, 156, 124, 241, 250, 51, 27, 8, 37, 111, 60, 187, 219, 161, 55, 42, 129,
  236,
]);

const UNWRAPPED_MINT_ADDRESS = address('F2qGWupzMUQnGfX8e25XZps8d9AGdVde8hLQT2pxsb4M');
const UNWRAPPED_TOKEN_ACCOUNT = address('94Y9pxekEm59b67PQQwvjb7wbwz689wDZ3dAwhCtJpPS'); // Must be owned by multisig account
const AMOUNT_TO_WRAP = 100n;

const main = async () => {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

  const payer = await createKeyPairSignerFromBytes(PAYER_KEYPAIR_BYTES);
  const { value: blockhash } = await rpc.getLatestBlockhash().send();

  // Initialize the wrapped mint
  const createMintMessage = await createMintTx({
    rpc,
    blockhash,
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
    blockhash,
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
    blockhash,
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

  const signerA = await createKeyPairSignerFromBytes(SIGNER_A_KEYPAIR_BYTES);
  const signerB = await createKeyPairSignerFromBytes(SIGNER_B_KEYPAIR_BYTES);

  // Two signers and the payer sign the transaction independently

  const wrapTxA = multisigOfflineSignWrapTx({
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

  const wrapTxB = multisigOfflineSignWrapTx({
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

  const wrapTxC = multisigOfflineSignWrapTx({
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

  const combinedTx = combinedMultisigWrapTx({
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
