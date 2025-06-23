use {
    crate::helpers::extension_initializer::ExtensionInitializer,
    solana_pubkey::Pubkey,
    spl_pod::{optional_keys::OptionalNonZeroPubkey, primitives::PodU64},
    spl_token_2022::{
        extension::{
            transfer_fee::{TransferFee, TransferFeeConfig},
            transfer_hook::TransferHook,
            BaseStateWithExtensionsMut, ExtensionType, PodStateWithExtensionsMut,
        },
        pod::PodMint,
    },
    std::convert::TryFrom,
};

pub struct TransferHookInit {
    pub program_id: Pubkey,
}

impl ExtensionInitializer<PodMint> for TransferHookInit {
    fn extension_type(&self) -> ExtensionType {
        ExtensionType::TransferHook
    }

    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<PodMint>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension = state.init_extension::<TransferHook>(false)?;
        extension.program_id = OptionalNonZeroPubkey::try_from(Some(self.program_id))?;
        Ok(())
    }
}

pub struct TransferFeeConfigInit {
    pub config: TransferFeeConfig,
}

impl ExtensionInitializer<PodMint> for TransferFeeConfigInit {
    fn extension_type(&self) -> ExtensionType {
        ExtensionType::TransferFeeConfig
    }

    fn initialize(
        &self,
        state: &mut PodStateWithExtensionsMut<PodMint>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let extension = state.init_extension::<TransferFeeConfig>(false)?;
        *extension = self.config;
        Ok(())
    }
}

pub struct TransferFeeConfigBuilder {
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
    transfer_fee_config_authority: Option<Pubkey>,
    withdraw_withheld_authority: Option<Pubkey>,
}

impl Default for TransferFeeConfigBuilder {
    fn default() -> Self {
        Self {
            transfer_fee_basis_points: 100, // 1%
            maximum_fee: 50_000,
            transfer_fee_config_authority: None,
            withdraw_withheld_authority: None,
        }
    }
}

impl TransferFeeConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn basis_points(mut self, basis_points: u16) -> Self {
        self.transfer_fee_basis_points = basis_points;
        self
    }

    pub fn maximum_fee(mut self, max_fee: u64) -> Self {
        self.maximum_fee = max_fee;
        self
    }

    pub fn config_authority(mut self, authority: Pubkey) -> Self {
        self.transfer_fee_config_authority = Some(authority);
        self
    }

    pub fn withdraw_authority(mut self, authority: Pubkey) -> Self {
        self.withdraw_withheld_authority = Some(authority);
        self
    }

    pub fn build(self) -> TransferFeeConfig {
        TransferFeeConfig {
            transfer_fee_config_authority: OptionalNonZeroPubkey::try_from(
                self.transfer_fee_config_authority
                    .or_else(|| Some(Pubkey::new_unique())),
            )
            .unwrap(),
            withdraw_withheld_authority: OptionalNonZeroPubkey::try_from(
                self.withdraw_withheld_authority
                    .or_else(|| Some(Pubkey::new_unique())),
            )
            .unwrap(),
            withheld_amount: PodU64::from(0),
            older_transfer_fee: TransferFee {
                epoch: 0.into(),
                maximum_fee: self.maximum_fee.into(),
                transfer_fee_basis_points: self.transfer_fee_basis_points.into(),
            },
            newer_transfer_fee: TransferFee {
                epoch: 0.into(),
                maximum_fee: self.maximum_fee.into(),
                transfer_fee_basis_points: self.transfer_fee_basis_points.into(),
            },
        }
    }
}
