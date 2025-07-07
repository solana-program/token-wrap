use {
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
            BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut,
        },
        pod::{PodAccount, PodMint},
    },
};

#[derive(Clone, Debug)]
pub enum MintExtension {
    ConfidentialTransfer,
    TransferHook,
    TransferFeeConfig,
    MintCloseAuthority(Pubkey),
    NonTransferable,
}

impl MintExtension {
    pub fn extension_type(&self) -> ExtensionType {
        match self {
            MintExtension::TransferHook => ExtensionType::TransferHook,
            MintExtension::TransferFeeConfig => ExtensionType::TransferFeeConfig,
            MintExtension::MintCloseAuthority(_) => ExtensionType::MintCloseAuthority,
            MintExtension::ConfidentialTransfer => ExtensionType::ConfidentialTransferMint,
            MintExtension::NonTransferable => ExtensionType::NonTransferable,
        }
    }
}

/// Initialize extensions for a mint account
pub fn init_mint_extensions(
    state: &mut PodStateWithExtensionsMut<PodMint>,
    extensions: &[MintExtension],
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
