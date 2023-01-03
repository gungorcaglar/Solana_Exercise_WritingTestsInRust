use crate::error::ReviewError;
use crate::instruction::MovieInstruction;
use crate::state::MovieAccountState;
use borsh::BorshSerialize;
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    borsh::try_from_slice_unchecked,
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::IsInitialized,
    pubkey::Pubkey,
    system_instruction,
    sysvar::{rent::Rent, Sysvar},
};
use std::convert::TryInto;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = MovieInstruction::unpack(instruction_data)?;
    match instruction {
        MovieInstruction::AddMovieReview {
            title,
            rating,
            description,
        } => add_movie_review(program_id, accounts, title, rating, description),
        MovieInstruction::UpdateMovieReview {
            title,
            rating,
            description,
        } => update_movie_review(program_id, accounts, title, rating, description)
    }
}

pub fn add_movie_review(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    title: String,
    rating: u8,
    description: String,
) -> ProgramResult {
    msg!("Adding movie review...");
    msg!("Title: {}", title);
    msg!("Rating: {}", rating);
    msg!("Description: {}", description);

    let account_info_iter = &mut accounts.iter();

    let initializer = next_account_info(account_info_iter)?;
    let pda_account = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;

    if !initializer.is_signer {
        msg!("Missing required signature");
        return Err(ProgramError::MissingRequiredSignature);
    }

    let (pda, bump_seed) = Pubkey::find_program_address(
        &[initializer.key.as_ref(), title.as_bytes().as_ref()],
        program_id,
    );
    if pda != *pda_account.key {
        msg!("Invalid seeds for PDA");
        return Err(ProgramError::InvalidArgument);
    }

    if rating > 5 || rating < 1 {
        msg!("Rating cannot be higher than 5");
        return Err(ReviewError::InvalidRating.into());
    }

    let total_len: usize = 1 + 1 + (4 + title.len()) + (4 + description.len());
    if total_len > 1000 {
        msg!("Data length is larger than 1000 bytes");
        return Err(ReviewError::InvalidDataLength.into());
    }

    let account_len: usize = 1000;

    let rent = Rent::get()?;
    let rent_lamports = rent.minimum_balance(account_len);

    invoke_signed(
        &system_instruction::create_account(
            initializer.key,
            pda_account.key,
            rent_lamports,
            account_len.try_into().unwrap(),
            program_id,
        ),
        &[
            initializer.clone(),
            pda_account.clone(),
            system_program.clone(),
        ],
        &[&[
            initializer.key.as_ref(),
            title.as_bytes().as_ref(),
            &[bump_seed],
        ]],
    )?;

    msg!("PDA created: {}", pda);

    msg!("unpacking state account");
    let mut account_data =
        try_from_slice_unchecked::<MovieAccountState>(&pda_account.data.borrow()).unwrap();
    msg!("borrowed account data");

    msg!("checking if movie account is already initialized");
    if account_data.is_initialized() {
        msg!("Account already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    account_data.reviewer = *initializer.key;
    account_data.title = title;
    account_data.rating = rating;
    account_data.description = description;
    account_data.is_initialized = true;

    msg!("serializing account");
    account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;
    msg!("state account serialized");

    Ok(())
}

pub fn update_movie_review(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    title: String,
    rating: u8,
    description: String,
) -> ProgramResult {
    msg!("Updating movie review...");

    let account_info_iter = &mut accounts.iter();

    let initializer = next_account_info(account_info_iter)?;
    let pda_account = next_account_info(account_info_iter)?;

    if pda_account.owner != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    if !initializer.is_signer {
        msg!("Missing required signature");
        return Err(ProgramError::MissingRequiredSignature);
    }

    msg!("unpacking state account");
    let mut account_data =
        try_from_slice_unchecked::<MovieAccountState>(&pda_account.data.borrow()).unwrap();
    msg!("review title: {}", account_data.title);

    let (pda, _bump_seed) = Pubkey::find_program_address(
        &[
            initializer.key.as_ref(),
            account_data.title.as_bytes().as_ref(),
        ],
        program_id,
    );
    if pda != *pda_account.key {
        msg!("Invalid seeds for PDA");
        return Err(ReviewError::InvalidPDA.into());
    }

    msg!("checking if movie account is initialized");
    if !account_data.is_initialized() {
        msg!("Account is not initialized");
        return Err(ReviewError::UninitializedAccount.into());
    }

    if rating > 5 || rating < 1 {
        msg!("Invalid Rating");
        return Err(ReviewError::InvalidRating.into());
    }

    let update_len: usize = 1 + 1 + (4 + description.len()) + account_data.title.len();
    if update_len > 1000 {
        msg!("Data length is larger than 1000 bytes");
        return Err(ReviewError::InvalidDataLength.into());
    }

    msg!("Review before update:");
    msg!("Title: {}", account_data.title);
    msg!("Rating: {}", account_data.rating);
    msg!("Description: {}", account_data.description);

    account_data.rating = rating;
    account_data.description = description;

    msg!("Review after update:");
    msg!("Title: {}", account_data.title);
    msg!("Rating: {}", account_data.rating);
    msg!("Description: {}", account_data.description);

    msg!("serializing account");
    account_data.serialize(&mut &mut pda_account.data.borrow_mut()[..])?;
    msg!("state account serialized");

    Ok(())
}

// Inside processor.rs
#[cfg(test)]
mod tests {
  use {
    super::*,
    assert_matches::*,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        system_program::ID as SYSTEM_PROGRAM_ID,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
        sysvar::rent::ID as SYSVAR_RENT_ID
    },
    spl_associated_token_account::{
        get_associated_token_address,
        instruction::create_associated_token_account,
    },
    spl_token:: ID as TOKEN_PROGRAM_ID,
  };

  // Inside the the tests modules
    fn create_init_mint_ix(payer: Pubkey, program_id: Pubkey) -> (Pubkey, Pubkey, Instruction) {
        // Derive PDA for token mint authority
        let (mint, _bump_seed) = Pubkey::find_program_address(&[b"token_mint"], &program_id);
        let (mint_auth, _bump_seed) = Pubkey::find_program_address(&[b"token_auth"], &program_id);
    
        let init_mint_ix = Instruction {
            program_id: program_id,
            accounts: vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(mint, false),
                AccountMeta::new(mint_auth, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(SYSVAR_RENT_ID, false)
            ],
            data: vec![3]
        };
    
        (mint, mint_auth, init_mint_ix)
    }


        // First unit test
    #[tokio::test]
    async fn test_initialize_mint_instruction() {
        let program_id = Pubkey::new_unique();
        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "pda_local",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;

        // Call helper function
        let (_mint, _mint_auth, init_mint_ix) = create_init_mint_ix(payer.pubkey(), program_id);

        // Create transaction object with instructions, accounts, and input data
        let mut transaction = Transaction::new_with_payer(
            &[init_mint_ix,],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);

        // Process transaction and compare the result
        assert_matches!(banks_client.process_transaction(transaction).await, Ok(_));
    }


    //Add movie review test

        // Second unit test
        //test add_movie_review instruction setup
    #[tokio::test]
    async fn test_add_movie_review_instruction() {
        let program_id = Pubkey::new_unique();
        let (mut banks_client, payer, recent_blockhash) = ProgramTest::new(
            "pda_local",
            program_id,
            processor!(process_instruction),
        )
        .start()
        .await;
        // Call helper function
        let (mint, mint_auth, init_mint_ix) = create_init_mint_ix(payer.pubkey(), program_id);


        /*derive PDAs (in second test)
        derive the review,
        comment counter
        user associated token account addresses.
        */

        // Create review PDA
        let title: String = "Captain America".to_owned();
        const RATING: u8 = 3;
        let review: String = "Liked the movie".to_owned();
        let (review_pda, _bump_seed) =
        Pubkey::find_program_address(&[payer.pubkey().as_ref(), title.as_bytes()], &program_id);

        // Create comment PDA
        let (comment_pda, _bump_seed) =
        Pubkey::find_program_address(&[review_pda.as_ref(), b"comment"], &program_id);

        // Create user associate token account of token mint
        let init_ata_ix: Instruction = create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &mint,
        );

        let user_ata: Pubkey =get_associated_token_address(&payer.pubkey(), &mint);


        //build transaction (still in second test)
        // Concat data to single buffer
        let mut data_vec = vec![0];
        data_vec.append(
            &mut (TryInto::<u32>::try_into(title.len()).unwrap().to_le_bytes())
                .try_into()
                .unwrap(),
        );
        data_vec.append(&mut title.into_bytes());
        data_vec.push(RATING);
        data_vec.append(
            &mut (TryInto::<u32>::try_into(review.len())
                .unwrap()
                .to_le_bytes())
            .try_into()
            .unwrap(),
        );
        data_vec.append(&mut review.into_bytes());

        // Create transaction object with instructions, accounts, and input data
        let mut transaction = Transaction::new_with_payer(
            &[
            init_mint_ix,
            init_ata_ix,
            Instruction {
                program_id: program_id,
                accounts: vec![
                    AccountMeta::new_readonly(payer.pubkey(), true),
                    AccountMeta::new(review_pda, false),
                    AccountMeta::new(comment_pda, false),
                    AccountMeta::new(mint, false),
                    AccountMeta::new_readonly(mint_auth, false),
                    AccountMeta::new(user_ata, false),
                    AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
                    AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
                ],
                data: data_vec,
            },
            ],
            Some(&payer.pubkey()),
        );
        transaction.sign(&[&payer], recent_blockhash);

        // Process transaction and compare the result
        assert_matches!(banks_client.process_transaction(transaction).await, Ok(_));
        
    }
  
}