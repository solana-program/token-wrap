/**
 * This code was AUTOGENERATED using the codama library.
 * Please DO NOT EDIT THIS FILE, instead use visitors
 * to add features, then rerun codama to update it.
 *
 * @see https://github.com/codama-idl/codama
 */

import {
  combineCodec,
  getBooleanDecoder,
  getBooleanEncoder,
  getStructDecoder,
  getStructEncoder,
  getU8Decoder,
  getU8Encoder,
  transformEncoder,
  type Address,
  type Codec,
  type Decoder,
  type Encoder,
  type IAccountMeta,
  type IInstruction,
  type IInstructionWithAccounts,
  type IInstructionWithData,
  type ReadonlyAccount,
  type WritableAccount,
} from '@solana/kit';
import { TOKEN_WRAP_PROGRAM_ADDRESS } from '../programs';
import { getAccountMetaFactory, type ResolvedAccount } from '../shared';

export const CREATE_MINT_DISCRIMINATOR = 0;

export function getCreateMintDiscriminatorBytes() {
  return getU8Encoder().encode(CREATE_MINT_DISCRIMINATOR);
}

export type CreateMintInstruction<
  TProgram extends string = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
  TAccountWrappedMint extends string | IAccountMeta<string> = string,
  TAccountBackpointer extends string | IAccountMeta<string> = string,
  TAccountUnwrappedMint extends string | IAccountMeta<string> = string,
  TAccountSystemProgram extends
    | string
    | IAccountMeta<string> = '11111111111111111111111111111111',
  TAccountWrappedTokenProgram extends string | IAccountMeta<string> = string,
  TRemainingAccounts extends readonly IAccountMeta<string>[] = [],
> = IInstruction<TProgram> &
  IInstructionWithData<Uint8Array> &
  IInstructionWithAccounts<
    [
      TAccountWrappedMint extends string
        ? WritableAccount<TAccountWrappedMint>
        : TAccountWrappedMint,
      TAccountBackpointer extends string
        ? WritableAccount<TAccountBackpointer>
        : TAccountBackpointer,
      TAccountUnwrappedMint extends string
        ? ReadonlyAccount<TAccountUnwrappedMint>
        : TAccountUnwrappedMint,
      TAccountSystemProgram extends string
        ? ReadonlyAccount<TAccountSystemProgram>
        : TAccountSystemProgram,
      TAccountWrappedTokenProgram extends string
        ? ReadonlyAccount<TAccountWrappedTokenProgram>
        : TAccountWrappedTokenProgram,
      ...TRemainingAccounts,
    ]
  >;

export type CreateMintInstructionData = {
  discriminator: number;
  /** Whether the creation should fail if the wrapped mint already exists. */
  idempotent: boolean;
};

export type CreateMintInstructionDataArgs = {
  /** Whether the creation should fail if the wrapped mint already exists. */
  idempotent?: boolean;
};

export function getCreateMintInstructionDataEncoder(): Encoder<CreateMintInstructionDataArgs> {
  return transformEncoder(
    getStructEncoder([
      ['discriminator', getU8Encoder()],
      ['idempotent', getBooleanEncoder()],
    ]),
    (value) => ({
      ...value,
      discriminator: CREATE_MINT_DISCRIMINATOR,
      idempotent: value.idempotent ?? false,
    })
  );
}

export function getCreateMintInstructionDataDecoder(): Decoder<CreateMintInstructionData> {
  return getStructDecoder([
    ['discriminator', getU8Decoder()],
    ['idempotent', getBooleanDecoder()],
  ]);
}

export function getCreateMintInstructionDataCodec(): Codec<
  CreateMintInstructionDataArgs,
  CreateMintInstructionData
> {
  return combineCodec(
    getCreateMintInstructionDataEncoder(),
    getCreateMintInstructionDataDecoder()
  );
}

export type CreateMintInput<
  TAccountWrappedMint extends string = string,
  TAccountBackpointer extends string = string,
  TAccountUnwrappedMint extends string = string,
  TAccountSystemProgram extends string = string,
  TAccountWrappedTokenProgram extends string = string,
> = {
  /**
   *  Unallocated wrapped mint account to create (PDA), address must be:
   * `get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`
   */
  wrappedMint: Address<TAccountWrappedMint>;
  /**
   * Unallocated wrapped backpointer account to create (PDA)
   * `get_wrapped_mint_backpointer_address(wrapped_mint_address)`
   */
  backpointer: Address<TAccountBackpointer>;
  /** The existing mint */
  unwrappedMint: Address<TAccountUnwrappedMint>;
  /** The system program */
  systemProgram?: Address<TAccountSystemProgram>;
  /** The token program used to create the wrapped mint */
  wrappedTokenProgram: Address<TAccountWrappedTokenProgram>;
  idempotent?: CreateMintInstructionDataArgs['idempotent'];
};

export function getCreateMintInstruction<
  TAccountWrappedMint extends string,
  TAccountBackpointer extends string,
  TAccountUnwrappedMint extends string,
  TAccountSystemProgram extends string,
  TAccountWrappedTokenProgram extends string,
  TProgramAddress extends Address = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
>(
  input: CreateMintInput<
    TAccountWrappedMint,
    TAccountBackpointer,
    TAccountUnwrappedMint,
    TAccountSystemProgram,
    TAccountWrappedTokenProgram
  >,
  config?: { programAddress?: TProgramAddress }
): CreateMintInstruction<
  TProgramAddress,
  TAccountWrappedMint,
  TAccountBackpointer,
  TAccountUnwrappedMint,
  TAccountSystemProgram,
  TAccountWrappedTokenProgram
> {
  // Program address.
  const programAddress = config?.programAddress ?? TOKEN_WRAP_PROGRAM_ADDRESS;

  // Original accounts.
  const originalAccounts = {
    wrappedMint: { value: input.wrappedMint ?? null, isWritable: true },
    backpointer: { value: input.backpointer ?? null, isWritable: true },
    unwrappedMint: { value: input.unwrappedMint ?? null, isWritable: false },
    systemProgram: { value: input.systemProgram ?? null, isWritable: false },
    wrappedTokenProgram: {
      value: input.wrappedTokenProgram ?? null,
      isWritable: false,
    },
  };
  const accounts = originalAccounts as Record<
    keyof typeof originalAccounts,
    ResolvedAccount
  >;

  // Original args.
  const args = { ...input };

  // Resolve default values.
  if (!accounts.systemProgram.value) {
    accounts.systemProgram.value =
      '11111111111111111111111111111111' as Address<'11111111111111111111111111111111'>;
  }

  const getAccountMeta = getAccountMetaFactory(programAddress, 'programId');
  const instruction = {
    accounts: [
      getAccountMeta(accounts.wrappedMint),
      getAccountMeta(accounts.backpointer),
      getAccountMeta(accounts.unwrappedMint),
      getAccountMeta(accounts.systemProgram),
      getAccountMeta(accounts.wrappedTokenProgram),
    ],
    programAddress,
    data: getCreateMintInstructionDataEncoder().encode(
      args as CreateMintInstructionDataArgs
    ),
  } as CreateMintInstruction<
    TProgramAddress,
    TAccountWrappedMint,
    TAccountBackpointer,
    TAccountUnwrappedMint,
    TAccountSystemProgram,
    TAccountWrappedTokenProgram
  >;

  return instruction;
}

export type ParsedCreateMintInstruction<
  TProgram extends string = typeof TOKEN_WRAP_PROGRAM_ADDRESS,
  TAccountMetas extends readonly IAccountMeta[] = readonly IAccountMeta[],
> = {
  programAddress: Address<TProgram>;
  accounts: {
    /**
     *  Unallocated wrapped mint account to create (PDA), address must be:
     * `get_wrapped_mint_address(unwrapped_mint_address, wrapped_token_program_id)`
     */

    wrappedMint: TAccountMetas[0];
    /**
     * Unallocated wrapped backpointer account to create (PDA)
     * `get_wrapped_mint_backpointer_address(wrapped_mint_address)`
     */

    backpointer: TAccountMetas[1];
    /** The existing mint */
    unwrappedMint: TAccountMetas[2];
    /** The system program */
    systemProgram?: TAccountMetas[3] | undefined;
    /** The token program used to create the wrapped mint */
    wrappedTokenProgram: TAccountMetas[4];
  };
  data: CreateMintInstructionData;
};

export function parseCreateMintInstruction<
  TProgram extends string,
  TAccountMetas extends readonly IAccountMeta[],
>(
  instruction: IInstruction<TProgram> &
    IInstructionWithAccounts<TAccountMetas> &
    IInstructionWithData<Uint8Array>
): ParsedCreateMintInstruction<TProgram, TAccountMetas> {
  if (instruction.accounts.length < 5) {
    // TODO: Coded error.
    throw new Error('Not enough accounts');
  }
  let accountIndex = 0;
  const getNextAccount = () => {
    const accountMeta = instruction.accounts![accountIndex]!;
    accountIndex += 1;
    return accountMeta;
  };
  const getNextOptionalAccount = () => {
    const accountMeta = getNextAccount();
    return accountMeta.address === TOKEN_WRAP_PROGRAM_ADDRESS
      ? undefined
      : accountMeta;
  };
  return {
    programAddress: instruction.programAddress,
    accounts: {
      wrappedMint: getNextAccount(),
      backpointer: getNextAccount(),
      unwrappedMint: getNextAccount(),
      systemProgram: getNextOptionalAccount(),
      wrappedTokenProgram: getNextAccount(),
    },
    data: getCreateMintInstructionDataDecoder().decode(instruction.data),
  };
}
