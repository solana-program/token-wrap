use spl_token_wrap::instruction::TokenWrapInstruction;

#[test]
fn test_pack_unpack_create_mint() {
    let instruction = TokenWrapInstruction::CreateMint { idempotent: true };
    let packed = instruction.pack();
    assert_eq!(packed, vec![0, 1]);

    let unpacked = TokenWrapInstruction::unpack(&packed).unwrap();
    assert_eq!(unpacked, instruction);

    let instruction = TokenWrapInstruction::CreateMint { idempotent: false };
    let packed = instruction.pack();
    assert_eq!(packed, vec![0, 0]);

    let unpacked = TokenWrapInstruction::unpack(&packed).unwrap();
    assert_eq!(unpacked, instruction);
}

#[test]
fn test_pack_unpack_wrap() {
    let instruction = TokenWrapInstruction::Wrap { amount: 42 };
    let packed = instruction.pack();
    assert_eq!(packed, vec![1, 42, 0, 0, 0, 0, 0, 0, 0]);

    let unpacked = TokenWrapInstruction::unpack(&packed).unwrap();
    assert_eq!(unpacked, instruction);
}

#[test]
fn test_pack_unpack_unwrap() {
    let instruction = TokenWrapInstruction::UnWrap { amount: 100 };
    let packed = instruction.pack();
    assert_eq!(packed, vec![2, 100, 0, 0, 0, 0, 0, 0, 0]);

    let unpacked = TokenWrapInstruction::unpack(&packed).unwrap();
    assert_eq!(unpacked, instruction);
}

#[test]
fn test_unpack_invalid_data() {
    assert!(TokenWrapInstruction::unpack(&[]).is_err());
    assert!(TokenWrapInstruction::unpack(&[3]).is_err());
    assert!(TokenWrapInstruction::unpack(&[0]).is_err());
    assert!(TokenWrapInstruction::unpack(&[1, 0, 0, 0]).is_err());
    assert!(TokenWrapInstruction::unpack(&[2, 0, 0, 0]).is_err());
    assert!(TokenWrapInstruction::unpack(&[0]).is_err());
    assert!(TokenWrapInstruction::unpack(&[0, 1, 0]).is_err());
}
