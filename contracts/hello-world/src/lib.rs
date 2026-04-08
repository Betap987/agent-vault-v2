#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    token::Client as TokenClient,
    Address, Env, Vec, Symbol,
};

// ═══════════════════════════════════════════════════
// ERRORES
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
// ESTRUCTURAS
// ═══════════════════════════════════════════════════
#[contracttype]
#[derive(Clone)]
pub struct VaultConfig {
    pub agent:          Address,
    pub token:          Address,
    pub spending_limit: i128,
    pub window_ledgers: u32,
    pub paused:         bool,
    pub failed_attempts: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct VaultState {
    pub balance:         i128,
    pub spent_in_window: i128,
    pub window_start:    u32,
}

// Vault Session — el OAuth de la ejecución financiera
#[contracttype]
#[derive(Clone)]
pub struct VaultSession {
    pub agent:        Address,  // agente autorizado para esta sesión
    pub budget:       i128,     // presupuesto total de la sesión
    pub spent:        i128,     // cuánto se ha gastado
    pub expires_at:   u32,      // ledger de expiración
    pub active:       bool,     // si la sesión está activa
}

// ═══════════════════════════════════════════════════
// CLAVES DE STORAGE — multiowner por Address
// ═══════════════════════════════════════════════════
#[contracttype]
pub enum DataKey {
    Config(Address),    // configuración por owner
    State(Address),     // estado por owner
    Whitelist(Address), // whitelist por owner
    Session(Address),   // sesión activa por owner
}

// ═══════════════════════════════════════════════════
// CONTRATO
// ═══════════════════════════════════════════════════
#[contract]
pub struct AgentVault;

#[contractimpl]
impl AgentVault {

    // ── INICIALIZAR VAULT ──────────────────────────
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

        // Evento: vault inicializado
        env.events().publish(
            (Symbol::new(&env, "vault_init"),),
            (owner.clone(),)
        );

        Ok(())
    }

    // ── DEPOSITAR ──────────────────────────────────
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

        // Evento: depósito registrado
        env.events().publish(
            (Symbol::new(&env, "deposit"),),
            (owner.clone(), amount)
        );

        Ok(())
    }

    // ── RETIRAR ────────────────────────────────────
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

        // Evento: retiro registrado
        env.events().publish(
            (Symbol::new(&env, "withdraw"),),
            (owner.clone(), amount)
        );

        Ok(())
    }

    // ── ABRIR VAULT SESSION ────────────────────────
    // El OAuth de la ejecución financiera
    pub fn open_session(
        env:            Env,
        owner:          Address,
        session_agent:  Address,
        budget:         i128,
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

        // Evento: sesión abierta
        env.events().publish(
            (Symbol::new(&env, "session_open"),),
            (owner.clone(), session_agent, budget, duration_ledgers)
        );

        Ok(())
    }

    // ── CERRAR VAULT SESSION ───────────────────────
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

        // Evento: sesión cerrada
        env.events().publish(
            (Symbol::new(&env, "session_close"),),
            (owner.clone(), session.spent)
        );

        Ok(())
    }

    // ── EXECUTE TRANSFER — policy engine completo ──
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

        // El agente firma
        config.agent.require_auth();

        // Funcion interna de autopausa
        let mut should_autopause = false;

        // POLICY ENGINE
        // 1. Vault activo
        if config.paused {
            return Ok(TransferResult::RejectedPaused);
        }

        // 2. Monto válido
        if amount <= 0 {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 {
                should_autopause = true;
            }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause {
                Self::autopause(&env, &owner, &mut config);
            }
            return Ok(TransferResult::RejectedInvalidAmount);
        }

        // 3. Whitelist
        if !whitelist.contains(&to) {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 {
                should_autopause = true;
            }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause {
                Self::autopause(&env, &owner, &mut config);
            }
            return Ok(TransferResult::RejectedNotWhitelisted);
        }

        // 4. Balance suficiente
        if amount > state.balance {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 {
                should_autopause = true;
            }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause {
                Self::autopause(&env, &owner, &mut config);
            }
            return Ok(TransferResult::RejectedInsufficientBalance);
        }

        // 5. Ventana de tiempo y límite
        let current_ledger = env.ledger().sequence();
        if current_ledger >= state.window_start + config.window_ledgers {
            state.spent_in_window = 0;
            state.window_start    = current_ledger;
        }

        if state.spent_in_window + amount > config.spending_limit {
            config.failed_attempts += 1;
            if config.failed_attempts >= 3 {
                should_autopause = true;
            }
            env.storage().persistent()
                .set(&DataKey::Config(owner.clone()), &config);
            if should_autopause {
                Self::autopause(&env, &owner, &mut config);
            }
            return Ok(TransferResult::RejectedLimitExceeded);
        }

        // Todo pasó — ejecutar pago
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

        // Evento: pago ejecutado
        env.events().publish(
            (Symbol::new(&env, "transfer"),),
            (owner.clone(), to, amount)
        );

        Ok(TransferResult::Success)
    }

    // ── EXECUTE SESSION TRANSFER — pago via sesión ─
    // Usado por agentes con vault session abierta
    // Es el flujo nativo para x402
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

        // El agente de la sesión firma
        session.agent.require_auth();

        // 1. Vault activo
        if config.paused {
            return Ok(TransferResult::RejectedPaused);
        }

        // 2. Sesión activa
        if !session.active {
            return Ok(TransferResult::RejectedSessionExpired);
        }

        // 3. Sesión no expirada
        let current_ledger = env.ledger().sequence();
        if current_ledger >= session.expires_at {
            session.active = false;
            env.storage().persistent()
                .set(&DataKey::Session(owner.clone()), &session);

            // Evento: sesión expirada
            env.events().publish(
                (Symbol::new(&env, "session_expired"),),
                (owner.clone(),)
            );

            return Ok(TransferResult::RejectedSessionExpired);
        }

        // 4. Presupuesto de sesión
        if session.spent + amount > session.budget {
            return Ok(TransferResult::RejectedSessionExhausted);
        }

        // 5. Monto válido
        if amount <= 0 {
            return Ok(TransferResult::RejectedInvalidAmount);
        }

        // 6. Balance del vault
        if amount > state.balance {
            return Ok(TransferResult::RejectedInsufficientBalance);
        }

        // Todo pasó — ejecutar pago
        let token_client = TokenClient::new(&env, &config.token);
        token_client.transfer(
            &env.current_contract_address(),
            &to,
            &amount,
        );

        state.balance  -= amount;
        session.spent  += amount;

        env.storage().persistent()
            .set(&DataKey::State(owner.clone()), &state);
        env.storage().persistent()
            .set(&DataKey::Session(owner.clone()), &session);

        // Evento: pago de sesión ejecutado
        env.events().publish(
            (Symbol::new(&env, "session_transfer"),),
            (owner.clone(), to, amount, session.spent, session.budget)
        );

        Ok(TransferResult::Success)
    }

    // ── ADMIN ──────────────────────────────────────
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
    pub fn get_config(
        env:   Env,
        owner: Address,
    ) -> Result<VaultConfig, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Config(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_state(
        env:   Env,
        owner: Address,
    ) -> Result<VaultState, VaultError> {
        env.storage().persistent()
            .get(&DataKey::State(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_whitelist(
        env:   Env,
        owner: Address,
    ) -> Result<Vec<Address>, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Whitelist(owner))
            .ok_or(VaultError::NotInitialized)
    }

    pub fn get_session(
        env:   Env,
        owner: Address,
    ) -> Result<VaultSession, VaultError> {
        env.storage().persistent()
            .get(&DataKey::Session(owner))
            .ok_or(VaultError::NoSession)
    }

    // ── FUNCIÓN INTERNA DE AUTOPAUSA ───────────────
    fn autopause(
        env:    &Env,
        owner:  &Address,
        config: &mut VaultConfig,
    ) {
        config.paused = true;
        env.storage().persistent()
            .set(&DataKey::Config(owner.clone()), config);

        env.events().publish(
            (Symbol::new(env, "auto_paused"),),
            (owner.clone(), config.failed_attempts)
        );
    }
}
