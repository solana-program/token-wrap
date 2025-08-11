use {
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    spl_pod::{optional_keys::OptionalNonZeroPubkey, primitives::PodU64},
    spl_token_2022::{
        extension::{
            confidential_transfer::ConfidentialTransferMint,
            immutable_owner::ImmutableOwner,
            mint_close_authority::MintCloseAuthority,
            non_transferable::{NonTransferable, NonTransferableAccount},
            transfer_fee::{TransferFee, TransferFeeAmount, TransferFeeConfig},
            transfer_hook::{TransferHook, TransferHookAccount},
            AccountType, BaseStateWithExtensionsMut, ExtensionType, Length,
            PodStateWithExtensionsMut,
        },
        pod::{PodAccount, PodMint},
        state::{Account, Mint},
    },
    spl_token_metadata_interface::state::TokenMetadata,
    spl_token_wrap::get_wrapped_mint_authority,
    spl_type_length_value::variable_len_pack::VariableLenPack,
};

#[derive(Clone, Debug)]
pub enum MintExtension {
    ConfidentialTransfer,
    TransferHook,
    TransferFeeConfig,
    MintCloseAuthority(Pubkey),
    NonTransferable,
    TokenMetadata {
        name: String,
        symbol: String,
        uri: String,
        additional_metadata: Vec<(String, String)>,
    },
    MetadataPointer,
}

impl MintExtension {
    pub fn extension_type(&self) -> ExtensionType {
        match self {
            MintExtension::TransferHook => ExtensionType::TransferHook,
            MintExtension::TransferFeeConfig => ExtensionType::TransferFeeConfig,
            MintExtension::MintCloseAuthority(_) => ExtensionType::MintCloseAuthority,
            MintExtension::ConfidentialTransfer => ExtensionType::ConfidentialTransferMint,
            MintExtension::NonTransferable => ExtensionType::NonTransferable,
            MintExtension::TokenMetadata { .. } => ExtensionType::TokenMetadata,
            MintExtension::MetadataPointer => ExtensionType::MetadataPointer,
        }
    }
}

/// Initialize extensions for a mint account
pub fn init_mint_extensions(
    state: &mut PodStateWithExtensionsMut<PodMint>,
    extensions: &[MintExtension],
    mint_key: &Pubkey,
) {
    for extension in extensions {
        match extension {
            MintExtension::TransferHook => {
                let extension = state.init_extension::<TransferHook>(false).unwrap();
                extension.program_id =
                    OptionalNonZeroPubkey::try_from(Some(test_transfer_hook::id())).unwrap();
            }
            MintExtension::TransferFeeConfig => {
                let extension = state.init_extension::<TransferFeeConfig>(false).unwrap();
                *extension = TransferFeeConfig {
                    transfer_fee_config_authority: OptionalNonZeroPubkey::try_from(Some(
                        Pubkey::new_unique(),
                    ))
                    .unwrap(),
                    withdraw_withheld_authority: OptionalNonZeroPubkey::try_from(Some(
                        Pubkey::new_unique(),
                    ))
                    .unwrap(),
                    withheld_amount: PodU64::from(0),
                    older_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: 50_000.into(),
                        transfer_fee_basis_points: 100.into(),
                    },
                    newer_transfer_fee: TransferFee {
                        epoch: 0.into(),
                        maximum_fee: 50_000.into(),
                        transfer_fee_basis_points: 100.into(),
                    },
                };
            }
            MintExtension::MintCloseAuthority(authority) => {
                let extension = state.init_extension::<MintCloseAuthority>(false).unwrap();
                extension.close_authority =
                    OptionalNonZeroPubkey::try_from(Some(*authority)).unwrap();
            }
            MintExtension::ConfidentialTransfer => {
                state
                    .init_extension::<ConfidentialTransferMint>(false)
                    .unwrap();
            }
            MintExtension::NonTransferable => {
                state.init_extension::<NonTransferable>(false).unwrap();
            }
            MintExtension::TokenMetadata {
                name,
                symbol,
                uri,
                additional_metadata,
            } => {
                let wrapped_mint_authority = get_wrapped_mint_authority(mint_key);
                let token_metadata = TokenMetadata {
                    update_authority: Some(wrapped_mint_authority).try_into().unwrap(),
                    mint: *mint_key,
                    name: name.clone(),
                    symbol: symbol.clone(),
                    uri: uri.clone(),
                    additional_metadata: additional_metadata.clone(),
                };
                state
                    .init_variable_len_extension::<TokenMetadata>(&token_metadata, false)
                    .unwrap();
            }
            MintExtension::MetadataPointer => {
                let wrapped_mint_authority = get_wrapped_mint_authority(mint_key);
                let extension = state
                    .init_extension::<spl_token_2022::extension::metadata_pointer::MetadataPointer>(
                        false,
                    )
                    .unwrap();
                extension.authority = Some(wrapped_mint_authority).try_into().unwrap();
                extension.metadata_address = Some(*mint_key).try_into().unwrap();
            }
        }
    }
}

/// Initialize extensions for a token account
pub fn init_token_account_extensions(
    state: &mut PodStateWithExtensionsMut<PodAccount>,
    extensions: &[ExtensionType],
) {
    for extension in extensions {
        match extension {
            ExtensionType::ImmutableOwner => {
                state.init_extension::<ImmutableOwner>(true).unwrap();
            }
            ExtensionType::TransferFeeAmount => {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
            }
            ExtensionType::TransferFeeConfig => {
                state.init_extension::<TransferFeeAmount>(true).unwrap();
            }
            ExtensionType::TransferHookAccount => {
                state.init_extension::<TransferHookAccount>(true).unwrap();
            }
            ExtensionType::NonTransferableAccount => {
                state
                    .init_extension::<NonTransferableAccount>(true)
                    .unwrap();
            }
            _ => unimplemented!(),
        }
    }
}

/// Calculate the exact mint account length including all extensions
pub fn calc_mint_len(mint_key: &Pubkey, extensions: &[MintExtension]) -> usize {
    if extensions.is_empty() {
        return Mint::LEN;
    }

    // Partition extensions into fixed-length and variable-length
    let (variable_len_extensions, fixed_len_extensions): (
        Vec<&MintExtension>,
        Vec<&MintExtension>,
    ) = extensions
        .iter()
        .partition(|e| matches!(e, MintExtension::TokenMetadata { .. }));

    let fixed_len_extension_types: Vec<ExtensionType> = fixed_len_extensions
        .iter()
        .map(|e| e.extension_type())
        .collect();

    // 1. Start with the size required for all fixed-length extensions.
    let mut len =
        ExtensionType::try_calculate_account_len::<Mint>(&fixed_len_extension_types).unwrap();

    // 2. If there were no fixed extensions, but there are variable extensions,
    // we need to account for the base padding and AccountType byte
    if fixed_len_extensions.is_empty() && !variable_len_extensions.is_empty() {
        len = Account::LEN.checked_add(size_of::<AccountType>()).unwrap();
    }

    // 3. Add size of variable-length extensions
    for extension in variable_len_extensions {
        let value_len = match extension {
            MintExtension::TokenMetadata {
                name,
                symbol,
                uri,
                additional_metadata,
            } => {
                let wrapped_mint_authority = get_wrapped_mint_authority(mint_key);
                let tm = TokenMetadata {
                    update_authority: Some(wrapped_mint_authority).try_into().unwrap(),
                    mint: *mint_key,
                    name: name.clone(),
                    symbol: symbol.clone(),
                    uri: uri.clone(),
                    additional_metadata: additional_metadata.clone(),
                };
                tm.get_packed_len().unwrap()
            }
            _ => panic!("should not happen due to partitioning"),
        };

        len = len
            .checked_add(size_of::<ExtensionType>())
            .unwrap()
            .checked_add(size_of::<Length>())
            .unwrap()
            .checked_add(value_len)
            .unwrap();
    }

    len
}
