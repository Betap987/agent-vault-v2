#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    token::Client as TokenClient,
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    Address, Env, Vec, Symbol, IntoVal,
};

// ═══════════════════════════════════════════════════
// ERRORS
// ═══════════════════════════════════════════════════
#[contracterror]
#[derive(Copy, Clone)]
pub enum VaultError {
    NotInitialized   = 1,
    AlreadyInit      = 2,
    Unauthorized     = 3,
    VaultPaused      = 4,
    NotWhitelisted   = 5,
    LimitExceeded    = 6,
    InsufficientBal  = 7,
    InvalidAmount    = 8,
    SessionExpired   = 9,
    SessionExhausted = 10,
    NoSession        = 11,
}

// ═══════════════════════════════════════════════════
// TRANSFER RESULT
// Typed result for execute_transfer — never fails
// silently, always returns a named outcome
// ═══════════════════════════════════════════════════
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum TransferResult {
    Success,
    RejectedNotWhitelisted,
    RejectedLimitExceeded,
    RejectedInsufficientBalance,
    RejectedInvalidAmount,
    RejectedPaused,
    RejectedSessionExpired,
    RejectedSessionExhausted,
}

// ═══════════════════════════════════════════════════
// STORAGE STRUCTURES
// ═══════════════════════════════════════════════════

// Vault configuration — set by owner, defines policies
#[contracttype]
#[derive(Clone)]
pub struct VaultConfig {
    pub agent:           Address,
    pub token:           Address,
    pub spending_limit:  i128,
    pub window_ledgers:  u32,
    pub paused:          bool,
    pub failed_attempts: u32,
}

// Vault state — changes with usage
#[contracttype]
#[derive(Clone)]
pub struct VaultState {
    pub balance:         i128,
    pub spent_in_window: i128,
    pub window_start:    u32,
}

// Vault Session — the OAuth of financial execution
#[contracttype]
#[derive(Clone)]
pub struct VaultSession {
    pub agent:      Address,
    pub budget:     i128,
    pub spent:      i128,
    pub expires_at: u32,
    pub active:     bool,
}

// ═══════════════════════════════════════════════════
// STORAGE KEYS — multiowner by Address
// ═══════════════════════════════════════════════════
#[contracttype]
pub enum DataKey {
    Config(Address),
    State(Address),
    Whitelist(Address),
    Session(Address),
}

// ═══════════════════════════════════════════════════
// CONTRACT
// ═══════════════════════════════════════════════════
#[contract]
pub struct AgentVault;

#[contractimpl]
impl AgentVault {

    // ── INITIALIZE VAULT ───────────────────────────
    pub fn init_vault(
        env:            Env,
        owner:          Address,
        agent:          Address,
        token:          Address,
        whitelist:      Vec<Address>,
        window_ledgers: u32,
        spending_limit: i128,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if env.storage().persistent()
            .has(&DataKey::Config(owner.clone())) {
            return Err(VaultError::AlreadyInit);
        }

        let config = VaultConfig {
            agent,
            token,
            spending_limit,
            window_ledgers,
            paused: false,
            failed_attempts: 0,
        };

        let state = VaultState {
            balance: 0,
            spent_in_window: 0,
            window_start: env.ledger().sequence(),
        };

        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), &config);
        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);
        env.storage().persistent()
            .set(&DataKey::Whitelist(owner.clone()), &whitelist);

        env.events().publish(
            (Symbol::new(&env, "vault_init"),),
            (owner.clone(),)
        );

        Ok(())
    }

    // ── DEPOSIT ────────────────────────────────────
    pub fn deposit(
        env:    Env,
        owner:  Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let token_client = TokenClient::new(&env, &config.token);
        token_client.transfer(
            &owner,
            &env.current_contract_address(),
            &amount,
        );

        state.balance += amount;
        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);

        env.events().publish(
            (Symbol::new(&env, "deposit"),),
            (owner.clone(), amount)
        );

        Ok(())
    }

    // ── WITHDRAW ───────────────────────────────────
    pub fn withdraw(
        env:    Env,
        owner:  Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        if amount > state.balance {
            return Err(VaultError::InsufficientBal);
        }

        let token_client = TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &owner,
            &amount,
        );

        state.balance -= amount;
        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);

        env.events().publish(
            (Symbol::new(&env, "withdraw"),),
            (owner.clone(), amount)
        );

        Ok(())
    }

    // ── DEPOSIT TO DEFINDEX ────────────────────────
    // Sends vault funds to DeFindex XLM vault to
    // generate yield while funds wait to be spent.
    // Uses authorize_as_current_contract to allow
    // DeFindex to pull tokens from this contract.
    pub fn deposit_to_defindex(
        env:    Env,
        owner:  Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        if amount > state.balance {
            return Err(VaultError::InsufficientBal);
        }

        // DeFindex XLM vault address on testnet
        let defindex_vault = Address::from_str(
            &env,
            "CCLV4H7WTLJQ7ATLHBBQV2WW3OINF3FOY5XZ7VPHZO7NH3D2ZS4GFSF6",
        );

        // Pre-authorize the sub-call that DeFindex will make
        // to transfer tokens from this contract to DeFindex
        env.authorize_as_current_contract(Vec::from_array(
            &env,
            [InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract:  config.token.clone(),
                    fn_name:   Symbol::new(&env, "transfer"),
                    args: (
                        env.current_contract_address(),
                        defindex_vault.clone(),
                        amount,
                    ).into_val(&env),
                },
                sub_invocations: Vec::new(&env),
            })],
        ));

        // Call DeFindex deposit
        let amounts_desired = Vec::from_array(&env, [amount]);
        let amounts_min     = Vec::from_array(&env, [0i128]);

        env.invoke_contract::<soroban_sdk::Val>(
    &defindex_vault,
    &Symbol::new(&env, "deposit"),
            Vec::from_array(&env, [
                amounts_desired.into_val(&env),
                amounts_min.into_val(&env),
                env.current_contract_address().into_val(&env),
                true.into_val(&env),
            ]),
        );

        state.balance -= amount;
        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);

        env.events().publish(
            (Symbol::new(&env, "defindex_deposit"),),
            (owner.clone(), amount)
        );

        Ok(())
    }

    // ── WITHDRAW FROM DEFINDEX ─────────────────────
    // Retrieves funds from DeFindex back into vault.
    // Returned amount includes original deposit plus
    // any yield generated while funds were deployed.
    pub fn withdraw_from_defindex(
        env:    Env,
        owner:  Address,
        shares: i128,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if shares <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let _config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        // DeFindex XLM vault address on testnet
        let defindex_vault = Address::from_str(
            &env,
            "CCLV4H7WTLJQ7ATLHBBQV2WW3OINF3FOY5XZ7VPHZO7NH3D2ZS4GFSF6",
        );

        let min_amounts_out = Vec::from_array(&env, [0i128]);

        let returned: Vec<i128> = env.invoke_contract(
            &defindex_vault,
            &Symbol::new(&env, "withdraw"),
            Vec::from_array(&env, [
                shares.into_val(&env),
                min_amounts_out.into_val(&env),
                env.current_contract_address().into_val(&env),
            ]),
        );

        let returned_amount = returned.get(0).unwrap_or(0);
        state.balance += returned_amount;
        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);

        env.events().publish(
            (Symbol::new(&env, "defindex_withdraw"),),
            (owner.clone(), returned_amount)
        );

        Ok(())
    }

    // ── OPEN VAULT SESSION ─────────────────────────
    // Creates a time-bounded, budget-limited access
    // grant for an agent. The OAuth of financial
    // execution on Stellar.
    pub fn open_session(
        env:              Env,
        owner:            Address,
        session_agent:    Address,
        budget:           i128,
        duration_ledgers: u32,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if budget <= 0 {
            return Err(VaultError::InvalidAmount);
        }

        let _config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        if budget > state.balance {
            return Err(VaultError::InsufficientBal);
        }

        let session = VaultSession {
            agent:      session_agent.clone(),
            budget,
            spent:      0,
            expires_at: env.ledger().sequence() + duration_ledgers,
            active:     true,
        };

        env.storage().persistent()
            .set(&DataKey::Session(owner.clone()), &session);

        env.events().publish(
            (Symbol::new(&env, "session_open"),),
            (owner.clone(), session_agent, budget, duration_ledgers)
        );

        Ok(())
    }

    // ── CLOSE VAULT SESSION ────────────────────────
    pub fn close_session(
        env:   Env,
        owner: Address,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        let mut session: VaultSession = env.storage().persistent()
            .get(&DataKey::Session(owner.clone()))
            .ok_or(VaultError::NoSession)?;

        session.active = false;
        env.storage().persistent()
            .set(&DataKey::Session(owner.clone()), &session);

        env.events().publish(
            (Symbol::new(&env, "session_close"),),
            (owner.clone(), session.spent)
        );

        Ok(())
    }

    // ── EXECUTE TRANSFER ───────────────────────────
    // Full policy engine for agent payments.
    // Returns typed TransferResult — never panics.
    // Auto-pauses after 3 consecutive failed attempts.
    pub fn execute_transfer(
        env:    Env,
        owner:  Address,
        to:     Address,
        amount: i128,
    ) -> Result<TransferResult, VaultError> {

        let mut config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let whitelist: Vec<Address> = env.storage().persistent()
            .get(&DataKey::Whitelist(owner.clone()))
            .unwrap_or(Vec::new(&env));

        config.agent.require_auth();

        let mut should_autopause = false;

        if config.paused {
            return Ok(TransferResult::RejectedPaused);
        }

        if amount <= 0 {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 { should_autopause = true; }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause { Self::autopause(&env, &owner, &mut config); }
            return Ok(TransferResult::RejectedInvalidAmount);
        }

        if !whitelist.contains(&to) {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 { should_autopause = true; }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause { Self::autopause(&env, &owner, &mut config); }
            return Ok(TransferResult::RejectedNotWhitelisted);
        }

        if amount > state.balance {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 { should_autopause = true; }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause { Self::autopause(&env, &owner, &mut config); }
            return Ok(TransferResult::RejectedInsufficientBalance);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger >= state.window_start + config.window_ledgers {
            state.spent_in_window = 0;
            state.window_start    = current_ledger;
        }

        if state.spent_in_window + amount > config.spending_limit {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 { should_autopause = true; }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause { Self::autopause(&env, &owner, &mut config); }
            return Ok(TransferResult::RejectedLimitExceeded);
        }

        let token_client = TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );

        state.balance         -= amount;
        state.spent_in_window += amount;
        config.failed_attempts = 0;

        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), &config);

        env.events().publish(
            (Symbol::new(&env, "transfer"),),
            (owner.clone(), to, amount)
        );

        Ok(TransferResult::Success)
    }

    // ── EXECUTE SESSION TRANSFER ───────────────────
    // Payment via vault session — native x402 flow.
    // Session budget and expiry enforced on-chain.
    pub fn execute_session_transfer(
        env:    Env,
        owner:  Address,
        to:     Address,
        amount: i128,
    ) -> Result<TransferResult, VaultError> {

        let config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut state: VaultState = env.storage().persistent()
            .get(&DataKey::State(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;

        let mut session: VaultSession = env.storage().persistent()
            .get(&DataKey::Session(owner.clone()))
            .ok_or(VaultError::NoSession)?;

        session.agent.require_auth();

        if config.paused {
            return Ok(TransferResult::RejectedPaused);
        }

        if !session.active {
            return Ok(TransferResult::RejectedSessionExpired);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger >= session.expires_at {
            session.active = false;
            env.storage().persistent()
                .set(&DataKey::Session(owner.clone()), &session);
            env.events().publish(
                (Symbol::new(&env, "session_expired"),),
                (owner.clone(),)
            );
            return Ok(TransferResult::RejectedSessionExpired);
        }

        if session.spent + amount > session.budget {
            return Ok(TransferResult::RejectedSessionExhausted);
        }

        if amount <= 0 {
            return Ok(TransferResult::RejectedInvalidAmount);
        }

        if amount > state.balance {
            return Ok(TransferResult::RejectedInsufficientBalance);
        }

        let token_client = TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );

        state.balance -= amount;
        session.spent += amount;

        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);
        env.storage().persistent()
            .set(&DataKey::Session(owner.clone()), &session);

        env.events().publish(
            (Symbol::new(&env, "session_transfer"),),
            (owner.clone(), to, amount, session.spent, session.budget)
        );

        Ok(TransferResult::Success)
    }

    // ── ADMIN FUNCTIONS ────────────────────────────

    pub fn set_paused(
        env:    Env,
        owner:  Address,
        paused: bool,
    ) -> Result<(), VaultError> {
        owner.require_auth();
        let mut config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;
        config.paused = paused;
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), &config);
        Ok(())
    }

    pub fn set_agent(
        env:       Env,
        owner:     Address,
        new_agent: Address,
    ) -> Result<(), VaultError> {
        owner.require_auth();
        let mut config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;
        config.agent = new_agent;
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), &config);
        Ok(())
    }

    pub fn set_whitelist(
        env:       Env,
        owner:     Address,
        whitelist: Vec<Address>,
    ) -> Result<(), VaultError> {
        owner.require_auth();
        if !env.storage().persistent()
            .has(&DataKey::Config(owner.clone())) {
            return Err(VaultError::NotInitialized);
        }
        env.storage().persistent()
            .set(&DataKey::Whitelist(owner.clone()), &whitelist);
        Ok(())
    }

    pub fn set_limits(
        env:            Env,
        owner:          Address,
        spending_limit: i128,
        window_ledgers: u32,
    ) -> Result<(), VaultError> {
        owner.require_auth();
        let mut config: VaultConfig = env.storage().persistent()
            .get(&DataKey::Config(owner.clone()))
            .ok_or(VaultError::NotInitialized)?;
        config.spending_limit = spending_limit;
        config.window_ledgers = window_ledgers;
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), &config);
        Ok(())
    }

    // ── READ ONLY ──────────────────────────────────

    pub fn get_config(env: Env, owner: Address) -> Result<VaultConfig, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Config(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_state(env: Env, owner: Address) -> Result<VaultState, VaultError> {
        env.storage().persistent()
            .get(&DataKey::State(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_whitelist(env: Env, owner: Address) -> Result<Vec<Address>, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Whitelist(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_session(env: Env, owner: Address) -> Result<VaultSession, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Session(owner))
            .ok_or(VaultError::NoSession)
    }

    // ── INTERNAL: AUTO-PAUSE ───────────────────────
    // Triggered after 3 consecutive failed attempts.
    // Emits auto_paused event for external monitoring.
    fn autopause(env: &Env, owner: &Address, config: &mut VaultConfig) {
        config.paused = true;
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), config);
        env.events().publish(
            (Symbol::new(env, "auto_paused"),),
            (owner.clone(), config.failed_attempts)
        );
    }
}