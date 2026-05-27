// HOOP Protocol v1.0 — FINAL DEPLOYMENT READY (May 2026)
// Soulbound Reputation + Generalized Verification Tasks
// Zero financialization — USDC value flow only

use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;

declare_id!("HoopProtocol1111111111111111111111111111111"); // ← CHANGE TO YOUR FINAL PROGRAM ID

#[program]
pub mod hoop_protocol {
    use super::*;

    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        community_mint: Pubkey,
        team_mint: Pubkey,
    ) -> Result<()> {
        let state = &mut ctx.accounts.protocol_state;
        state.authority = *ctx.accounts.authority.key;
        state.community_mint = community_mint;
        state.team_mint = team_mint;
        state.bump = ctx.bumps.protocol_state;
        Ok(())
    }

    pub fn register_validator(ctx: Context<RegisterValidator>) -> Result<()> {
        let profile = &mut ctx.accounts.validator_profile;
        profile.owner = ctx.accounts.validator.key();
        profile.reputation_score = 100;
        profile.tasks_validated = 0;
        profile.total_usdc_earned = 0;
        profile.is_active = true;
        profile.registered_at = Clock::get()?.unix_timestamp;
        profile.bump = ctx.bumps.validator_profile;

        emit!(ValidatorRegistered {
            validator: profile.owner,
            initial_reputation: profile.reputation_score,
        });
        Ok(())
    }

    pub fn create_verification_task(
        ctx: Context<CreateVerificationTask>,
        task_id: String,
        description: String,
        usdc_fee: u64,
        required_reputation: u64,
    ) -> Result<()> {
        let task = &mut ctx.accounts.task;
        task.task_id = task_id;
        task.creator = ctx.accounts.creator.key();
        task.description = description;
        task.usdc_fee = usdc_fee;
        task.required_reputation = required_reputation;
        task.status = TaskStatus::Open;
        task.created_at = Clock::get()?.unix_timestamp;
        task.bump = ctx.bumps.task;
        Ok(())
    }

    pub fn claim_task(ctx: Context<ClaimTask>) -> Result<()> {
        let task = &mut ctx.accounts.task;
        let validator = &ctx.accounts.validator_profile;

        require!(task.status == TaskStatus::Open, ErrorCode::TaskNotOpen);
        require!(validator.reputation_score >= task.required_reputation, ErrorCode::InsufficientReputation);
        require!(validator.is_active, ErrorCode::ValidatorInactive);

        task.status = TaskStatus::Claimed;
        task.validator = Some(validator.owner);
        task.claimed_at = Some(Clock::get()?.unix_timestamp);
        Ok(())
    }

    pub fn submit_task_attestation(
        ctx: Context<SubmitTaskAttestation>,
        attestation_hash: [u8; 32],
        success: bool,
    ) -> Result<()> {
        let task = &mut ctx.accounts.task;
        let validator = &mut ctx.accounts.validator_profile;
        let state = &mut ctx.accounts.protocol_state;

        require!(task.status == TaskStatus::Claimed, ErrorCode::TaskNotClaimed);
        require!(task.validator == Some(validator.owner), ErrorCode::NotAssignedValidator);

        task.completion_attestation = Some(Attestation {
            hash: attestation_hash,
            submitted_by: ctx.accounts.submitter.key(),
            timestamp: Clock::get()?.unix_timestamp,
            success,
        });

        if success {
            task.status = TaskStatus::Completed;
            validator.tasks_validated = validator.tasks_validated.saturating_add(1);
            validator.reputation_score = validator.reputation_score.saturating_add(10);
            validator.total_usdc_earned = validator.total_usdc_earned.saturating_add(task.usdc_fee);

            state.total_tasks_completed = state.total_tasks_completed.saturating_add(1);
            state.total_usdc_routed = state.total_usdc_routed.saturating_add(task.usdc_fee);

            emit!(TaskCompleted {
                task_id: task.task_id.clone(),
                validator: validator.owner,
                usdc_fee: task.usdc_fee,
                new_reputation: validator.reputation_score,
            });
        } else {
            task.status = TaskStatus::Failed;
            validator.reputation_score = validator.reputation_score.saturating_sub(5);
        }
        Ok(())
    }
}

// ==================== ACCOUNTS ====================
#[account]
pub struct ProtocolState {
    pub authority: Pubkey,
    pub community_mint: Pubkey,
    pub team_mint: Pubkey,
    pub total_validators: u64,
    pub total_tasks_completed: u64,
    pub total_usdc_routed: u64,
    pub bump: u8,
}

#[account]
pub struct ValidatorProfile {
    pub owner: Pubkey,
    pub reputation_score: u64,
    pub tasks_validated: u64,
    pub total_usdc_earned: u64,
    pub is_active: bool,
    pub registered_at: i64,
    pub bump: u8,
}

#[account]
pub struct VerificationTask {
    pub task_id: String,
    pub creator: Pubkey,
    pub description: String,
    pub usdc_fee: u64,
    pub required_reputation: u64,
    pub status: TaskStatus,
    pub created_at: i64,
    pub claimed_at: Option<i64>,
    pub validator: Option<Pubkey>,
    pub completion_attestation: Option<Attestation>,
    pub bump: u8,
}

// ==================== ENUMS & STRUCTS ====================
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum TaskStatus { Open, Claimed, Completed, Failed }

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Attestation {
    pub hash: [u8; 32],
    pub submitted_by: Pubkey,
    pub timestamp: i64,
    pub success: bool,
}

// ==================== EVENTS ====================
#[event]
pub struct ValidatorRegistered { pub validator: Pubkey; pub initial_reputation: u64; }

#[event]
pub struct TaskCompleted {
    pub task_id: String,
    pub validator: Pubkey,
    pub usdc_fee: u64,
    pub new_reputation: u64,
}

// ==================== ERRORS ====================
#[error_code]
pub enum ErrorCode {
    #[msg("Task is not open")]
    TaskNotOpen,
    #[msg("Insufficient reputation")]
    InsufficientReputation,
    #[msg("Validator inactive")]
    ValidatorInactive,
    #[msg("Task not claimed")]
    TaskNotClaimed,
    #[msg("Not the assigned validator")]
    NotAssignedValidator,
}

// ==================== CONTEXTS ====================
#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(init, payer = authority, space = 8 + ProtocolState::LEN, seeds = [b"protocol"], bump)]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterValidator<'info> {
    #[account(init, payer = validator, space = 8 + ValidatorProfile::LEN, seeds = [b"validator", validator.key().as_ref()], bump)]
    pub validator_profile: Account<'info, ValidatorProfile>,
    #[account(mut)]
    pub protocol_state: Account<'info, ProtocolState>,
    #[account(mut)]
    pub validator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(task_id: String)]
pub struct CreateVerificationTask<'info> {
    #[account(init, payer = creator, space = 8 + VerificationTask::LEN, seeds = [b"task", task_id.as_bytes()], bump)]
    pub task: Account<'info, VerificationTask>,
    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimTask<'info> {
    #[account(mut)]
    pub task: Account<'info, VerificationTask>,
    pub validator_profile: Account<'info, ValidatorProfile>,
}

#[derive(Accounts)]
pub struct SubmitTaskAttestation<'info> {
    #[account(mut)]
    pub task: Account<'info, VerificationTask>,
    #[account(mut)]
    pub validator_profile: Account<'info, ValidatorProfile>,
    pub submitter: Signer<'info>,
    pub protocol_state: Account<'info, ProtocolState>,
}