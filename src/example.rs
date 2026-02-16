use crate::prelude::*;

#[derive(Accounts)]
pub struct Make<'info> {
    pub maker: &'info mut Signer,
    pub escrow: &'info mut Initialize<EscrowAccount>,
    pub maker_ta_a: &'info mut Account<TokenAccount>,
    pub maker_ta_b: &'info Account<TokenAccount>,
    pub vault_ta_a: &'info Account<TokenAccount>,
    pub rent: &'info Rent,
    pub token_program: &'info TokenProgram,
    pub system_program: &'info SystemProgram,
}

#[instruction(discriminator = 0)]
pub fn make(ctx: Ctx<Make>, receive: u64) -> Result<(), ProgramError> {
    EscrowAccount {
        maker: ctx.accounts.maker.to_account_view().address().clone(),
        mint_a: ctx.accounts.maker_ta_a.mint().clone(),
        mint_b: ctx.accounts.maker_ta_b.mint().clone(),
        maker_ta_b: ctx.accounts.maker_ta_b.to_account_view().address().clone(),
        receive
    }.init(
        ctx.accounts.escrow,
        ctx.accounts.maker.to_account_view(),
        &ctx.accounts.rent
    )
}

#[account(discriminator = 1)]
pub struct EscrowAccount {
    pub maker: Address,
    pub mint_a: Address,
    pub mint_b: Address,
    pub maker_ta_b: Address,
    pub receive: u64,
}
