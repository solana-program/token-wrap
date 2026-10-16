import { findAssociatedTokenPda } from '@solana-program/token-2022';
import {
    Address,
    appendTransactionMessageInstructions,
    Blockhash,
    createTransactionMessage,
    GetAccountInfoApi,
    HasAddress,
    Instruction,
    pipe,
    Rpc,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    TransactionMessage,
    TransactionMessageWithBlockhashLifetime,
    TransactionMessageWithFeePayerSigner,
    TransactionSigner,
} from '@solana/kit';
import { type InstructionAccountInput, type InstructionSignerInput } from '@solana/program-client-core';
import { findWrappedMintAuthorityPda, findWrappedMintPda, getWrapInstruction, WrapInput } from './generated';
import { toAddress } from './internal';
import { getMintFromTokenAccount, getOwnerFromAccount } from './utilities';

interface IxBuilderArgs {
    unwrappedTokenAccount: Address | HasAddress;
    wrappedTokenProgram: Address | HasAddress;
    amount: bigint | number;
    wrappedMint: Address | HasAddress;
    wrappedMintAuthority: Address | HasAddress;
    transferAuthority: InstructionAccountInput | InstructionSignerInput;
    unwrappedMint: Address | HasAddress;
    recipientWrappedTokenAccount: Address | HasAddress;
    unwrappedTokenProgram: Address | HasAddress;
    multiSigners?: TransactionSigner[];
}

export interface MultiSignerWrapIxBuilderArgs extends IxBuilderArgs {
    payer: TransactionSigner;
    blockhash: {
        blockhash: Blockhash;
        lastValidBlockHeight: bigint;
    };
    multiSigners: TransactionSigner[];
}

// Used to collect signatures
export async function multisigOfflineSignWrap(
    args: MultiSignerWrapIxBuilderArgs,
): Promise<TransactionMessage & TransactionMessageWithBlockhashLifetime & TransactionMessageWithFeePayerSigner> {
    const wrapIx = await buildWrapIx(args);

    return pipe(
        createTransactionMessage({ version: 0 }),
        tx => setTransactionMessageFeePayerSigner(args.payer, tx),
        tx => setTransactionMessageLifetimeUsingBlockhash(args.blockhash, tx),
        tx => appendTransactionMessageInstructions([wrapIx], tx),
    );
}

export interface SingleSignerWrapArgs {
    rpc: Rpc<GetAccountInfoApi>;
    payer: TransactionSigner; // Fee payer and default transfer authority
    unwrappedTokenAccount: Address | HasAddress;
    wrappedTokenProgram: Address | HasAddress;
    amount: bigint | number;
    transferAuthority?: InstructionAccountInput | InstructionSignerInput; // Defaults to payer if not provided
    unwrappedMint?: Address | HasAddress; // Will fetch from unwrappedTokenAccount if not provided
    recipientWrappedTokenAccount?: Address | HasAddress; // Defaults to payer's ATA if not provided
    unwrappedTokenProgram?: Address | HasAddress; // Will fetch from unwrappedTokenAccount owner if not provided
}

export interface SingleSignerWrapResult {
    ixs: Instruction[];
    recipientWrappedTokenAccount: Address;
    escrowAccount: Address;
    amount: bigint;
}

export async function singleSignerWrap({
    rpc,
    payer,
    unwrappedTokenAccount: inputUnwrappedTokenAccount,
    wrappedTokenProgram: inputWrappedTokenProgram,
    amount,
    transferAuthority: inputTransferAuthority,
    unwrappedMint: inputUnwrappedMint,
    recipientWrappedTokenAccount: inputRecipientTokenAccount,
    unwrappedTokenProgram: inputUnwrappedTokenProgram,
}: SingleSignerWrapArgs): Promise<SingleSignerWrapResult> {
    const unwrappedTokenAccount = toAddress(inputUnwrappedTokenAccount);
    const wrappedTokenProgram = toAddress(inputWrappedTokenProgram);
    const {
        unwrappedMint,
        unwrappedTokenProgram,
        wrappedMint,
        wrappedMintAuthority,
        recipientWrappedTokenAccount,
        transferAuthority,
        unwrappedEscrow,
    } = await resolveAddrs({
        rpc,
        payer,
        inputTransferAuthority,
        inputUnwrappedMint: inputUnwrappedMint && toAddress(inputUnwrappedMint),
        unwrappedTokenAccount,
        inputUnwrappedTokenProgram: inputUnwrappedTokenProgram && toAddress(inputUnwrappedTokenProgram),
        wrappedTokenProgram,
        inputRecipientTokenAccount: inputRecipientTokenAccount && toAddress(inputRecipientTokenAccount),
    });

    const ix = await buildWrapIx({
        unwrappedTokenAccount,
        wrappedTokenProgram,
        amount,
        transferAuthority,
        unwrappedMint,
        wrappedMint,
        wrappedMintAuthority,
        recipientWrappedTokenAccount,
        unwrappedTokenProgram,
    });

    return {
        ixs: [ix],
        recipientWrappedTokenAccount,
        escrowAccount: unwrappedEscrow,
        amount: BigInt(amount),
    };
}

// Meant to handle all of the potential default values
async function resolveAddrs({
    rpc,
    payer,
    unwrappedTokenAccount,
    wrappedTokenProgram,
    inputTransferAuthority,
    inputUnwrappedMint,
    inputRecipientTokenAccount,
    inputUnwrappedTokenProgram,
}: {
    rpc: Rpc<GetAccountInfoApi>;
    payer: TransactionSigner;
    unwrappedTokenAccount: Address;
    wrappedTokenProgram: Address;
    inputTransferAuthority?: InstructionAccountInput | InstructionSignerInput;
    inputUnwrappedMint?: Address;
    inputRecipientTokenAccount?: Address;
    inputUnwrappedTokenProgram?: Address;
}) {
    const unwrappedMint = inputUnwrappedMint ?? (await getMintFromTokenAccount(rpc, unwrappedTokenAccount));
    const unwrappedTokenProgram = inputUnwrappedTokenProgram ?? (await getOwnerFromAccount(rpc, unwrappedTokenAccount));
    const [wrappedMint] = await findWrappedMintPda({ unwrappedMint, wrappedTokenProgram });
    const [wrappedMintAuthority] = await findWrappedMintAuthorityPda({ wrappedMint });
    const recipientWrappedTokenAccount =
        inputRecipientTokenAccount ??
        (
            await findAssociatedTokenPda({
                owner: payer.address,
                mint: wrappedMint,
                tokenProgram: wrappedTokenProgram,
            })
        )[0];
    const [unwrappedEscrow] = await findAssociatedTokenPda({
        owner: wrappedMintAuthority,
        mint: unwrappedMint,
        tokenProgram: unwrappedTokenProgram,
    });

    const transferAuthority = inputTransferAuthority ?? payer;

    return {
        unwrappedEscrow,
        transferAuthority,
        unwrappedMint,
        unwrappedTokenProgram,
        wrappedMint,
        wrappedMintAuthority,
        recipientWrappedTokenAccount,
    };
}

async function buildWrapIx({
    unwrappedTokenAccount,
    wrappedTokenProgram,
    amount,
    transferAuthority,
    unwrappedMint,
    recipientWrappedTokenAccount,
    unwrappedTokenProgram,
    wrappedMint,
    wrappedMintAuthority,
    multiSigners = [],
}: IxBuilderArgs): Promise<Instruction> {
    const [unwrappedEscrow] = await findAssociatedTokenPda({
        owner: wrappedMintAuthority,
        mint: unwrappedMint,
        tokenProgram: unwrappedTokenProgram,
    });

    const wrapInstructionInput: WrapInput = {
        recipientWrappedTokenAccount,
        wrappedMint,
        wrappedMintAuthority,
        unwrappedTokenProgram,
        wrappedTokenProgram,
        unwrappedTokenAccount,
        unwrappedMint,
        unwrappedEscrow,
        transferAuthority,
        amount: BigInt(amount),
        multiSigners,
    };

    return getWrapInstruction(wrapInstructionInput);
}
