import {
  address,
  createKeyPairSignerFromBytes,
  createNoopSigner,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  getBase58Decoder,
} from '@solana/kit';
import { TOKEN_2022_PROGRAM_ADDRESS } from '@solana-program/token-2022';
import { findWrappedMintAuthorityPda, findWrappedMintPda } from '../generated';
import { multisigOfflineSignWrap } from '../wrap';
import {
  createEscrowAccount,
  createTokenAccount,
  getOwnerFromAccount,
  multisigBroadcast,
} from '../utilities';
import { executeCreateMint } from '../create-mint';
import { multisigOfflineSignUnwrap } from '../unwrap';

// Replace these consts with your own
const PAYER_KEYPAIR_BYTES = new Uint8Array([
  242, 30, 38, 177, 152, 71, 235, 193, 93, 30, 119, 131, 42, 186, 202, 7, 45, 250, 126, 135, 107,
  137, 38, 91, 202, 212, 12, 8, 154, 213, 163, 200, 23, 237, 17, 163, 3, 135, 34, 126, 235, 146,
  251, 18, 199, 101, 153, 249, 134, 88, 219, 68, 167, 136, 234, 195, 12, 34, 184, 85, 234, 25, 125,
  94,
]);

// Create using CLI: spl-token create-multisig 2 $SIGNER_1_PUBKEY $SIGNER_2_PUBKEY
const MULTISIG_PUBKEY_SPL_TOKEN = address('4ofshdhToSz56LLwTrZH7TrUnVJVk3uEgLQLQmYcZynF');
const MULTISIG_PUBKEY_SPL_TOKEN_2022 = address('B4zpMNng3noSj8tF8Sxu1FBMRcCRHHcLUypaquqKsiyn');
const SIGNER_A_KEYPAIR_BYTES = new Uint8Array([
  77, 131, 162, 241, 28, 96, 241, 189, 123, 127, 7, 219, 35, 85, 12, 88, 193, 190, 213, 204, 199,
  77, 116, 81, 115, 19, 74, 195, 204, 44, 131, 184, 153, 236, 12, 139, 32, 129, 221, 90, 26, 106,
  30, 242, 54, 167, 146, 214, 199, 62, 64, 68, 227, 95, 113, 236, 13, 140, 113, 222, 221, 120, 169,
  122,
]);
const SIGNER_B_KEYPAIR_BYTES = new Uint8Array([
  35, 38, 214, 119, 234, 198, 186, 126, 191, 31, 81, 169, 59, 193, 231, 194, 61, 89, 72, 115, 21,
  160, 41, 85, 25, 35, 61, 134, 221, 207, 177, 245, 84, 168, 63, 96, 104, 70, 30, 42, 148, 66, 148,
  229, 191, 138, 23, 59, 149, 133, 213, 104, 150, 140, 91, 158, 35, 176, 5, 99, 14, 68, 184, 16,
]);

const UNWRAPPED_MINT_ADDRESS = address('E8r9ixwg7QYr6xCh4tSdHErZ6CUxQhVGHqF5bRoZXyyV');
const UNWRAPPED_TOKEN_ACCOUNT = address('DGNyuKAWP3susy6XMbVsYHy2AMrrKmh8pXM3WpQUeyL2'); // Must be owned by multisig account
const AMOUNT_TO_WRAP = 100n;

const main = async () => {
  const rpc = createSolanaRpc('http://127.0.0.1:8899');
  const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
  const payer = await createKeyPairSignerFromBytes(PAYER_KEYPAIR_BYTES);

  // Initialize the wrapped mint
  const createMintResult = await executeCreateMint({
    rpc,
    rpcSubscriptions,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    payer,
    idempotent: true,
  });
  console.log('======== Create Mint Successful ========');
  console.log('Wrapped Mint:', createMintResult.wrappedMint);
  console.log('Backpointer:', createMintResult.backpointer);
  console.log('Funded wrapped mint lamports:', createMintResult.fundedWrappedMintLamports);
  console.log('Funded backpointer lamports:', createMintResult.fundedBackpointerLamports);
  console.log('Signature:', createMintResult.signature);

  // Setup accounts needed for wrap
  const escrowAccount = await createEscrowAccount({
    rpc,
    rpcSubscriptions,
    payer,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });

  const [wrappedMint] = await findWrappedMintPda({
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
  });
  const recipientWrappedTokenAccount = await createTokenAccount({
    rpc,
    rpcSubscriptions,
    payer,
    mint: wrappedMint,
    tokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    owner: MULTISIG_PUBKEY_SPL_TOKEN_2022,
  });

  const unwrappedTokenProgram = await getOwnerFromAccount(rpc, UNWRAPPED_TOKEN_ACCOUNT);
  const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });

  const { value: wrapBlockhash } = await rpc.getLatestBlockhash().send();

  const signerA = await createKeyPairSignerFromBytes(SIGNER_A_KEYPAIR_BYTES);
  const signerB = await createKeyPairSignerFromBytes(SIGNER_B_KEYPAIR_BYTES);

  // Two signers and the payer sign the transaction independently

  const wrapSignatureMapA = await multisigOfflineSignWrap({
    payer: payer.address,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [signerA, createNoopSigner(signerB.address)],
    blockhash: wrapBlockhash,
  });

  const wrapSignatureMapB = await multisigOfflineSignWrap({
    payer: payer.address,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), signerB],
    blockhash: wrapBlockhash,
  });

  const wrapSignatureMapC = await multisigOfflineSignWrap({
    payer: payer,
    unwrappedTokenAccount: UNWRAPPED_TOKEN_ACCOUNT,
    escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    recipientWrappedTokenAccount,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), createNoopSigner(signerB.address)],
    blockhash: wrapBlockhash,
  });

  // Lastly, all signatures are combined together and broadcast

  const wrapTx = await multisigBroadcast({
    rpc,
    rpcSubscriptions,
    signedTxs: [wrapSignatureMapA, wrapSignatureMapB, wrapSignatureMapC],
    blockhash: wrapBlockhash,
  });

  console.log('======== Multisig Wrap Successful ========');
  for (const [pubkey, signature] of Object.entries(wrapTx.signatures)) {
    if (signature) {
      const base58Sig = getBase58Decoder().decode(signature);
      console.log(`pubkey: ${pubkey}`);
      console.log(`signature: ${base58Sig}`);
      console.log('-----');
    }
  }

  // Because the `recipientWrappedTokenAccount` is owned by the multisig,
  // we'll need to use the multisig functions to wrap the position

  const { value: unwrapBlockhash } = await rpc.getLatestBlockhash().send();

  const unwrapSignatureMapA = await multisigOfflineSignUnwrap({
    payer: payer.address,
    unwrappedEscrow: escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenAccount: recipientWrappedTokenAccount,
    recipientUnwrappedToken: UNWRAPPED_TOKEN_ACCOUNT,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN_2022,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [signerA, createNoopSigner(signerB.address)],
    blockhash: unwrapBlockhash,
  });

  const unwrapSignatureMapB = await multisigOfflineSignUnwrap({
    payer: payer.address,
    unwrappedEscrow: escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenAccount: recipientWrappedTokenAccount,
    recipientUnwrappedToken: UNWRAPPED_TOKEN_ACCOUNT,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN_2022,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), signerB],
    blockhash: unwrapBlockhash,
  });

  const unwrapSignatureMapC = await multisigOfflineSignUnwrap({
    payer: payer,
    unwrappedEscrow: escrowAccount,
    wrappedTokenProgram: TOKEN_2022_PROGRAM_ADDRESS,
    amount: AMOUNT_TO_WRAP,
    unwrappedMint: UNWRAPPED_MINT_ADDRESS,
    wrappedTokenAccount: recipientWrappedTokenAccount,
    recipientUnwrappedToken: UNWRAPPED_TOKEN_ACCOUNT,
    transferAuthority: MULTISIG_PUBKEY_SPL_TOKEN_2022,
    wrappedMint,
    wrappedMintAuthority,
    unwrappedTokenProgram,
    multiSigners: [createNoopSigner(signerA.address), createNoopSigner(signerB.address)],
    blockhash: unwrapBlockhash,
  });

  const unwrapTx = await multisigBroadcast({
    rpc,
    rpcSubscriptions,
    signedTxs: [unwrapSignatureMapA, unwrapSignatureMapB, unwrapSignatureMapC],
    blockhash: unwrapBlockhash,
  });

  console.log('======== Multisig Unwrap Successful ========');
  for (const [pubkey, signature] of Object.entries(unwrapTx.signatures)) {
    if (signature) {
      const base58Sig = getBase58Decoder().decode(signature);
      console.log(`pubkey: ${pubkey}`);
      console.log(`signature: ${base58Sig}`);
      console.log('-----');
    }
  }
};

void main();
