use {
    crate::helpers::mint_builder::TokenProgram,
    mollusk_svm::Mollusk,
    solana_account::Account,
    solana_program_option::COption,
    solana_program_pack::Pack,
    solana_pubkey::Pubkey,
    solana_rent::Rent,
    spl_pod::{
        optional_keys::OptionalNonZeroPubkey,
        primitives::{PodBool, PodU64},
    },
    spl_token_2022::{
        extension::{
            mint_close_authority::MintCloseAuthority, BaseStateWithExtensionsMut, ExtensionType,
            PodStateWithExtensionsMut,
        },
        pod::{PodCOption, PodMint},
    },
    std::convert::TryFrom,
};

pub fn init_mollusk() -> Mollusk {
    let mut mollusk = Mollusk::new(&spl_token_wrap::id(), "spl_token_wrap");
    mollusk_svm_programs_token::token::add_program(&mut mollusk);
    mollusk_svm_programs_token::token2022::add_program(&mut mollusk);
    mollusk
}

pub const MINT_DECIMALS: u8 = 12;
pub const MINT_SUPPLY: u64 = 500_000_000;
pub const FREEZE_AUTHORITY: Pubkey =
    Pubkey::from_str_const("11111115q4EpJaTXAZWpCg3J2zppWGSZ46KXozzo9");

fn token_2022_with_extension_data(supply: u64) -> Vec<u8> {
    let mint_size =
        ExtensionType::try_calculate_account_len::<PodMint>(&[ExtensionType::MintCloseAuthority])
            .unwrap();
    let mut buffer = vec![0; mint_size];
    let mut state =
        PodStateWithExtensionsMut::<PodMint>::unpack_uninitialized(&mut buffer).unwrap();
    state.base.decimals = MINT_DECIMALS;
    state.base.is_initialized = PodBool::from_bool(true);
    state.base.supply = PodU64::from(supply);
    state.base.freeze_authority = PodCOption::from(COption::Some(FREEZE_AUTHORITY));
    state.init_account_type().unwrap();

    let extension = state.init_extension::<MintCloseAuthority>(true).unwrap();
    let close_authority =
        OptionalNonZeroPubkey::try_from(Some(Pubkey::new_from_array([1; 32]))).unwrap();
    extension.close_authority = close_authority;

    buffer
}

// Spl_token and token_2022 are the same account structure except for owner
pub fn setup_mint(token_program: TokenProgram, rent: &Rent, mint_authority: Pubkey) -> Account {
    let state = spl_token::state::Mint {
        decimals: MINT_DECIMALS,
        is_initialized: true,
        supply: MINT_SUPPLY,
        mint_authority: COption::Some(mint_authority),
        freeze_authority: COption::Some(FREEZE_AUTHORITY),
    };
    let mut data = match token_program {
        TokenProgram::SplToken => vec![0u8; spl_token::state::Mint::LEN],
        TokenProgram::Token2022 => token_2022_with_extension_data(MINT_SUPPLY),
    };
    state.pack_into_slice(&mut data);

    let lamports = rent.minimum_balance(data.len());

    Account {
        lamports,
        data,
        owner: token_program.id(),
        ..Default::default()
    }
}
