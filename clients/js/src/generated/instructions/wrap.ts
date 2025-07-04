/**
 * This code was AUTOGENERATED using the codama library.
 * Please DO NOT EDIT THIS FILE, instead use visitors
 * to add features, then rerun codama to update it.
 *
 * @see https://github.com/codama-idl/codama
 */

import {
  AccountRole,
  combineCodec,
  getStructDecoder,
  getStructEncoder,
  getU64Decoder,
  getU64Encoder,
  getU8Decoder,
  getU8Encoder,
  transformEncoder,
  type Address,
  type Codec,
  type Decoder,
  type Encoder,
  type IAccountMeta,
  type IAccountSignerMeta,
  type IInstruction,
  type IInstructionWithAccounts,
  type IInstructionWithData,
  type ReadonlyAccount,
  type ReadonlySignerAccount,
  type TransactionSigner,
  type WritableAccount,
} from '@solana/kit';
import { TOKEN_WRAP_PROGRAM_ADDRESS } from '../programs';
import { getAccountMetaFactory, type ResolvedAccount } from '../shared';

export const WRAP_DISCRIMINATOR = 1;

export function getWrapDiscriminatorBytes() {
  return getU8Encoder().encode(WRAP_DISCRIMINATOR);
}

export type WrapInstruction<
  TProgram extends string = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
  TAccountRecipientWrappedTokenAccount extends
    | string
    | IAccountMeta<string> = string,
  TAccountWrappedMint extends string | IAccountMeta<string> = string,
  TAccountWrappedMintAuthority extends string | IAccountMeta<string> = string,
  TAccountUnwrappedTokenProgram extends string | IAccountMeta<string> = string,
  TAccountWrappedTokenProgram extends string | IAccountMeta<string> = string,
  TAccountUnwrappedTokenAccount extends string | IAccountMeta<string> = string,
  TAccountUnwrappedMint extends string | IAccountMeta<string> = string,
  TAccountUnwrappedEscrow extends string | IAccountMeta<string> = string,
  TAccountTransferAuthority extends string | IAccountMeta<string> = string,
  TRemainingAccounts extends readonly IAccountMeta<string>[] = [],
> = IInstruction<TProgram> &
  IInstructionWithData<Uint8Array> &
  IInstructionWithAccounts<
    [
      TAccountRecipientWrappedTokenAccount extends string
        ? WritableAccount<TAccountRecipientWrappedTokenAccount>
        : TAccountRecipientWrappedTokenAccount,
      TAccountWrappedMint extends string
        ? WritableAccount<TAccountWrappedMint>
        : TAccountWrappedMint,
      TAccountWrappedMintAuthority extends string
        ? ReadonlyAccount<TAccountWrappedMintAuthority>
        : TAccountWrappedMintAuthority,
      TAccountUnwrappedTokenProgram extends string
        ? ReadonlyAccount<TAccountUnwrappedTokenProgram>
        : TAccountUnwrappedTokenProgram,
      TAccountWrappedTokenProgram extends string
        ? ReadonlyAccount<TAccountWrappedTokenProgram>
        : TAccountWrappedTokenProgram,
      TAccountUnwrappedTokenAccount extends string
        ? WritableAccount<TAccountUnwrappedTokenAccount>
        : TAccountUnwrappedTokenAccount,
      TAccountUnwrappedMint extends string
        ? ReadonlyAccount<TAccountUnwrappedMint>
        : TAccountUnwrappedMint,
      TAccountUnwrappedEscrow extends string
        ? WritableAccount<TAccountUnwrappedEscrow>
        : TAccountUnwrappedEscrow,
      TAccountTransferAuthority extends string
        ? ReadonlyAccount<TAccountTransferAuthority>
        : TAccountTransferAuthority,
      ...TRemainingAccounts,
    ]
  >;

export type WrapInstructionData = {
  discriminator: number;
  /** The amount of tokens to wrap. */
  amount: bigint;
};

export type WrapInstructionDataArgs = {
  /** The amount of tokens to wrap. */
  amount: number | bigint;
};

export function getWrapInstructionDataEncoder(): Encoder<WrapInstructionDataArgs> {
  return transformEncoder(
    getStructEncoder([
      ['discriminator', getU8Encoder()],
      ['amount', getU64Encoder()],
    ]),
    (value) => ({ ...value, discriminator: WRAP_DISCRIMINATOR })
  );
}

export function getWrapInstructionDataDecoder(): Decoder<WrapInstructionData> {
  return getStructDecoder([
    ['discriminator', getU8Decoder()],
    ['amount', getU64Decoder()],
  ]);
}

export function getWrapInstructionDataCodec(): Codec<
  WrapInstructionDataArgs,
  WrapInstructionData
> {
  return combineCodec(
    getWrapInstructionDataEncoder(),
    getWrapInstructionDataDecoder()
  );
}

export type WrapInput<
  TAccountRecipientWrappedTokenAccount extends string = string,
  TAccountWrappedMint extends string = string,
  TAccountWrappedMintAuthority extends string = string,
  TAccountUnwrappedTokenProgram extends string = string,
  TAccountWrappedTokenProgram extends string = string,
  TAccountUnwrappedTokenAccount extends string = string,
  TAccountUnwrappedMint extends string = string,
  TAccountUnwrappedEscrow extends string = string,
  TAccountTransferAuthority extends string = string,
> = {
  /** The token account to receive the wrapped tokens. */
  recipientWrappedTokenAccount: Address<TAccountRecipientWrappedTokenAccount>;
  /**
   * Wrapped mint, must be initialized, address must be:
   * `get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`
   */
  wrappedMint: Address<TAccountWrappedMint>;
  /**
   * The PDA authority of the wrapped mint, address must be
   * `get_wrapped_mint_authority(wrapped_mint)`
   */
  wrappedMintAuthority: Address<TAccountWrappedMintAuthority>;
  /** The token program of the unwrapped tokens. */
  unwrappedTokenProgram: Address<TAccountUnwrappedTokenProgram>;
  /** The token program of the wrapped tokens. */
  wrappedTokenProgram: Address<TAccountWrappedTokenProgram>;
  /** The source token account for the unwrapped tokens. */
  unwrappedTokenAccount: Address<TAccountUnwrappedTokenAccount>;
  /** The mint of the unwrapped tokens. */
  unwrappedMint: Address<TAccountUnwrappedMint>;
  /**
   * The escrow account that holds the unwrapped tokens.
   * Address must be ATA: get_escrow_address(unwrapped_mint, unwrapped_token_program, wrapped_token_program)
   */
  unwrappedEscrow: Address<TAccountUnwrappedEscrow>;
  /** The authority to transfer the unwrapped tokens. */
  transferAuthority:
    | Address<TAccountTransferAuthority>
    | TransactionSigner<TAccountTransferAuthority>;
  amount: WrapInstructionDataArgs['amount'];
  multiSigners?: Array<TransactionSigner>;
};

export function getWrapInstruction<
  TAccountRecipientWrappedTokenAccount extends string,
  TAccountWrappedMint extends string,
  TAccountWrappedMintAuthority extends string,
  TAccountUnwrappedTokenProgram extends string,
  TAccountWrappedTokenProgram extends string,
  TAccountUnwrappedTokenAccount extends string,
  TAccountUnwrappedMint extends string,
  TAccountUnwrappedEscrow extends string,
  TAccountTransferAuthority extends string,
  TProgramAddress extends Address = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
>(
  input: WrapInput<
    TAccountRecipientWrappedTokenAccount,
    TAccountWrappedMint,
    TAccountWrappedMintAuthority,
    TAccountUnwrappedTokenProgram,
    TAccountWrappedTokenProgram,
    TAccountUnwrappedTokenAccount,
    TAccountUnwrappedMint,
    TAccountUnwrappedEscrow,
    TAccountTransferAuthority
  >,
  config?: { programAddress?: TProgramAddress }
): WrapInstruction<
  TProgramAddress,
  TAccountRecipientWrappedTokenAccount,
  TAccountWrappedMint,
  TAccountWrappedMintAuthority,
  TAccountUnwrappedTokenProgram,
  TAccountWrappedTokenProgram,
  TAccountUnwrappedTokenAccount,
  TAccountUnwrappedMint,
  TAccountUnwrappedEscrow,
  (typeof input)['transferAuthority'] extends TransactionSigner<TAccountTransferAuthority>
    ? ReadonlySignerAccount<TAccountTransferAuthority> &
        IAccountSignerMeta<TAccountTransferAuthority>
    : TAccountTransferAuthority
> {
  // Program address.
  const programAddress = config?.programAddress ?? TOKEN_WRAP_PROGRAM_ADDRESS;

  // Original accounts.
  const originalAccounts = {
    recipientWrappedTokenAccount: {
      value: input.recipientWrappedTokenAccount ?? null,
      isWritable: true,
    },
    wrappedMint: { value: input.wrappedMint ?? null, isWritable: true },
    wrappedMintAuthority: {
      value: input.wrappedMintAuthority ?? null,
      isWritable: false,
    },
    unwrappedTokenProgram: {
      value: input.unwrappedTokenProgram ?? null,
      isWritable: false,
    },
    wrappedTokenProgram: {
      value: input.wrappedTokenProgram ?? null,
      isWritable: false,
    },
    unwrappedTokenAccount: {
      value: input.unwrappedTokenAccount ?? null,
      isWritable: true,
    },
    unwrappedMint: { value: input.unwrappedMint ?? null, isWritable: false },
    unwrappedEscrow: { value: input.unwrappedEscrow ?? null, isWritable: true },
    transferAuthority: {
      value: input.transferAuthority ?? null,
      isWritable: false,
    },
  };
  const accounts = originalAccounts as Record<
    keyof typeof originalAccounts,
    ResolvedAccount
  >;

  // Original args.
  const args = { ...input };

  // Remaining accounts.
  const remainingAccounts: IAccountMeta[] = (args.multiSigners ?? []).map(
    (signer) => ({
      address: signer.address,
      role: AccountRole.READONLY_SIGNER,
      signer,
    })
  );

  const getAccountMeta = getAccountMetaFactory(programAddress, 'programId');
  const instruction = {
    accounts: [
      getAccountMeta(accounts.recipientWrappedTokenAccount),
      getAccountMeta(accounts.wrappedMint),
      getAccountMeta(accounts.wrappedMintAuthority),
      getAccountMeta(accounts.unwrappedTokenProgram),
      getAccountMeta(accounts.wrappedTokenProgram),
      getAccountMeta(accounts.unwrappedTokenAccount),
      getAccountMeta(accounts.unwrappedMint),
      getAccountMeta(accounts.unwrappedEscrow),
      getAccountMeta(accounts.transferAuthority),
      ...remainingAccounts,
    ],
    programAddress,
    data: getWrapInstructionDataEncoder().encode(
      args as WrapInstructionDataArgs
    ),
  } as WrapInstruction<
    TProgramAddress,
    TAccountRecipientWrappedTokenAccount,
    TAccountWrappedMint,
    TAccountWrappedMintAuthority,
    TAccountUnwrappedTokenProgram,
    TAccountWrappedTokenProgram,
    TAccountUnwrappedTokenAccount,
    TAccountUnwrappedMint,
    TAccountUnwrappedEscrow,
    (typeof input)['transferAuthority'] extends TransactionSigner<TAccountTransferAuthority>
      ? ReadonlySignerAccount<TAccountTransferAuthority> &
          IAccountSignerMeta<TAccountTransferAuthority>
      : TAccountTransferAuthority
  >;

  return instruction;
}

export type ParsedWrapInstruction<
  TProgram extends string = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
  TAccountMetas extends readonly IAccountMeta[] = readonly IAccountMeta[],
> = {
  programAddress: Address<TProgram>;
  accounts: {
    /** The token account to receive the wrapped tokens. */
    recipientWrappedTokenAccount: TAccountMetas[0];
    /**
     * Wrapped mint, must be initialized, address must be:
     * `get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`
     */

    wrappedMint: TAccountMetas[1];
    /**
     * The PDA authority of the wrapped mint, address must be
     * `get_wrapped_mint_authority(wrapped_mint)`
     */

    wrappedMintAuthority: TAccountMetas[2];
    /** The token program of the unwrapped tokens. */
    unwrappedTokenProgram: TAccountMetas[3];
    /** The token program of the wrapped tokens. */
    wrappedTokenProgram: TAccountMetas[4];
    /** The source token account for the unwrapped tokens. */
    unwrappedTokenAccount: TAccountMetas[5];
    /** The mint of the unwrapped tokens. */
    unwrappedMint: TAccountMetas[6];
    /**
     * The escrow account that holds the unwrapped tokens.
     * Address must be ATA: get_escrow_address(unwrapped_mint, unwrapped_token_program, wrapped_token_program)
     */

    unwrappedEscrow: TAccountMetas[7];
    /** The authority to transfer the unwrapped tokens. */
    transferAuthority: TAccountMetas[8];
  };
  data: WrapInstructionData;
};

export function parseWrapInstruction<
  TProgram extends string,
  TAccountMetas extends readonly IAccountMeta[],
>(
  instruction: IInstruction<TProgram> &
    IInstructionWithAccounts<TAccountMetas> &
    IInstructionWithData<Uint8Array>
): ParsedWrapInstruction<TProgram, TAccountMetas> {
  if (instruction.accounts.length < 9) {
    // TODO: Coded error.
    throw new Error('Not enough accounts');
  }
  let accountIndex = 0;
  const getNextAccount = () => {
    const accountMeta = instruction.accounts![accountIndex]!;
    accountIndex += 1;
    return accountMeta;
  };
  return {
    programAddress: instruction.programAddress,
    accounts: {
      recipientWrappedTokenAccount: getNextAccount(),
      wrappedMint: getNextAccount(),
      wrappedMintAuthority: getNextAccount(),
      unwrappedTokenProgram: getNextAccount(),
      wrappedTokenProgram: getNextAccount(),
      unwrappedTokenAccount: getNextAccount(),
      unwrappedMint: getNextAccount(),
      unwrappedEscrow: getNextAccount(),
      transferAuthority: getNextAccount(),
    },
    data: getWrapInstructionDataDecoder().decode(instruction.data),
  };
}
