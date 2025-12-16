use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_token::instruction as token_instruction;

entrypoint!(process_instruction);

#[derive(Debug)]
enum SwapInstruction {
    Swap { amount: u64 },
}

impl SwapInstruction {
    fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        if input.is_empty() {
            return Err(ProgramError::InvalidInstructionData);
        }

        let (tag, rest) = input.split_first().ok_or(ProgramError::InvalidInstructionData)?;

        match tag {
            0 => {
                if rest.len() < 8 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let amount = u64::from_le_bytes(
                    rest[..8]
                        .try_into()
                        .map_err(|_| ProgramError::InvalidInstructionData)?,
                );
                Ok(SwapInstruction::Swap { amount })
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Token Swap Program: Processing instruction");

    let instruction = SwapInstruction::unpack(instruction_data)?;

    match instruction {
        SwapInstruction::Swap { amount } => {
            msg!("Swapping {} tokens", amount);
            swap_tokens(program_id, accounts, amount)
        }
    }
}

fn swap_tokens(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    amount: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();

    let source_account = next_account_info(account_info_iter)?;
    let dest_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;

    // Verify authority is a signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    msg!(
        "Transferring {} tokens from {} to {}",
        amount,
        source_account.key,
        dest_account.key
    );

    // In a real implementation, you would:
    // 1. Transfer tokens from source to destination
    // 2. Handle swap logic (e.g., calculate exchange rate)
    // 3. Transfer tokens back (if needed)
    // 4. Update swap pool state (if using AMM)
    
    // For this example, we'll just log the swap operation
    // A full implementation would use spl_token::instruction::transfer
    
    msg!("Swap completed successfully!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };
    use solana_sdk::{
        message::Message,
        signature::{Keypair, Signer},
        transaction::Transaction,
    };

    #[test]
    fn test_swap_instruction_unpack() {
        let amount: u64 = 1000;
        let mut data = vec![0u8]; // Swap instruction tag
        data.extend_from_slice(&amount.to_le_bytes());

        let instruction = SwapInstruction::unpack(&data).unwrap();
        match instruction {
            SwapInstruction::Swap { amount: unpacked_amount } => {
                assert_eq!(unpacked_amount, amount);
            }
        }
    }
}
