import {
  address,
  createKeyPairSignerFromBytes,
  createNoopSigner,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getBase58Decoder,
  getSignatureFromTransaction,
  sendAndConfirmTransactionFactory,
  signTransactionMessageWithSigners,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, findWrappedMintPda } from '../generated';
import { multisigBroadcastWrap, multisigOfflineSignWrap } from '../wrap';
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
const MULTISIG_PUBKEY = address('A9MUtNSeGvAzpcntPYQg5vzt1Q5Xc1zDLcCW7tAySkAN');
const SIGNER_A_KEYPAIR_BYTES = new Uint8Array([
  114, 129, 55, 122, 217, 194, 64, 230, 140, 159, 22, 38, 99, 12, 92, 182, 65, 7, 54, 134, 88, 157,
  91, 63, 152, 228, 94, 67, 87, 46, 94, 15, 76, 174, 18, 113, 84, 204, 184, 69, 235, 46, 42, 32,
  215, 223, 193, 2, 69, 166, 120, 188, 225, 232, 124, 110, 7, 246, 244, 11, 58, 198, 200, 10,
]);
const SIGNER_B_KEYPAIR_BYTES = new Uint8Array([
  38, 135, 51, 17, 62, 14, 47, 243, 191, 43, 224, 7, 121, 116, 129, 220, 153, 157, 25, 89, 138, 31,
  244, 202, 53, 149, 110, 16, 74, 160, 227, 109, 145, 179, 77, 135, 239, 34, 214, 103, 92, 56, 145,
  4, 178, 162, 166, 37, 40, 75, 178, 111, 89, 79, 251, 230, 180, 210, 158, 176, 97, 102, 4, 197,
]);

const UNWRAPPED_MINT_ADDRESS = address('9aDExopzFYZMkm1GqrevQMaAWi6gX9du1xEmQAATkE8j');
const UNWRAPPED_TOKEN_ACCOUNT = address('3JK5rrQ6nR6EkrgeLKiPnjsx16muPoPt3AQGBc77jgqE'); // Must be owned by multisig account
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

  const signatureMapA = await multisigOfflineSignWrap({
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

  const signatureMapB = await multisigOfflineSignWrap({
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

  const signatureMapC = await multisigOfflineSignWrap({
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

  // Lastly, all signatures are combined together and broadcast

  const transaction = await multisigBroadcastWrap({
    rpc,
    rpcSubscriptions,
    signedTxs: [signatureMapA, signatureMapB, signatureMapC],
    blockhash,
  });

  console.log('======== Confirmed Multisig Tx ✅ ========');
  for (const [pubkey, signature] of Object.entries(transaction.signatures)) {
    if (signature) {
      const base58Sig = getBase58Decoder().decode(signature);
      console.log(`pubkey: ${pubkey}`);
      console.log(`signature: ${base58Sig}`);
      console.log('-----');
    }
  }
};

void main();
