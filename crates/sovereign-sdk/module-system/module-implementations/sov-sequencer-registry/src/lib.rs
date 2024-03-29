//! The `sov-sequencer-registry` module is responsible for sequencer
//! registration, slashing, and rewards. At the moment, only a centralized
//! sequencer is supported. The sequencer's address and bond are registered
//! during the rollup deployment.
//!
//! The module implements the [`sov_modules_api::hooks::ApplyBlobHooks`] trait.

#![deny(missing_docs)]
mod call;
mod genesis;
mod hooks;
#[cfg(feature = "native")]
mod query;
pub use call::*;
pub use genesis::*;
#[cfg(feature = "native")]
pub use query::*;
use sov_modules_api::prelude::*;
use sov_modules_api::{CallResponse, Error, ModuleInfo, StateMap, StateValue, WorkingSet};
use sov_state::codec::BcsCodec;

/// The `sov-sequencer-registry` module `struct`.
#[cfg_attr(feature = "native", derive(sov_modules_api::ModuleCallJsonSchema))]
#[derive(Clone, ModuleInfo)]
pub struct SequencerRegistry<C: sov_modules_api::Context, Da: sov_modules_api::DaSpec> {
    /// The address of the `sov_sequencer_registry` module.
    /// Note: this is address is generated by the module framework and the
    /// corresponding private key is unknown.
    #[address]
    pub(crate) address: C::Address,

    /// Reference to the Bank module.
    #[module]
    pub(crate) bank: sov_bank::Bank<C>,

    /// Only batches from sequencers from this list are going to be processed.
    #[state]
    pub(crate) allowed_sequencers: StateMap<Da::Address, C::Address, BcsCodec>,

    /// Optional preferred sequencer.
    /// If set, batches from this sequencer will be processed first in block,
    /// So this sequencer can guarantee soft confirmation time for transactions
    #[state]
    pub(crate) preferred_sequencer: StateValue<Da::Address, BcsCodec>,

    /// Coin's that will be slashed if the sequencer is malicious.
    /// The coins will be transferred from
    /// [`SequencerConfig::seq_rollup_address`] to
    /// [`SequencerRegistry::address`] and locked forever, until sequencer
    /// decides to exit (unregister).
    ///
    /// Only sequencers in the [`SequencerRegistry::allowed_sequencers`] list are
    /// allowed to exit.
    #[state]
    pub(crate) coins_to_lock: StateValue<sov_bank::Coins<C>>,
}

/// Result of applying a blob, from sequencer's point of view.
pub enum SequencerOutcome<Da: sov_modules_api::DaSpec> {
    /// The blob was applied successfully and the operation is concluded.
    Completed,
    /// The blob was *not* applied successfully. The sequencer has been slashed
    /// as a result of the invalid blob.
    Slashed {
        /// The address of the sequencer that was slashed.
        sequencer: Da::Address,
    },
}

impl<C: sov_modules_api::Context, Da: sov_modules_api::DaSpec> sov_modules_api::Module
    for SequencerRegistry<C, Da>
{
    type Context = C;

    type Config = SequencerConfig<C, Da>;

    type CallMessage = CallMessage;

    type Event = ();

    fn genesis(&self, config: &Self::Config, working_set: &mut WorkingSet<C>) -> Result<(), Error> {
        Ok(self.init_module(config, working_set)?)
    }

    fn call(
        &self,
        message: Self::CallMessage,
        context: &Self::Context,
        working_set: &mut WorkingSet<C>,
    ) -> Result<CallResponse, Error> {
        Ok(match message {
            CallMessage::Register { da_address } => {
                let da_address = Da::Address::try_from(&da_address)?;
                self.register(&da_address, context, working_set)?
            }
            CallMessage::Exit { da_address } => {
                let da_address = Da::Address::try_from(&da_address)?;
                self.exit(&da_address, context, working_set)?
            }
        })
    }
}

impl<C: sov_modules_api::Context, Da: sov_modules_api::DaSpec> SequencerRegistry<C, Da> {
    /// Returns the configured amount of [`Coins`](sov_bank::Coins) to lock.
    pub fn get_coins_to_lock(&self, working_set: &mut WorkingSet<C>) -> Option<sov_bank::Coins<C>> {
        self.coins_to_lock.get(working_set)
    }

    pub(crate) fn register_sequencer(
        &self,
        da_address: &Da::Address,
        rollup_address: &C::Address,
        working_set: &mut WorkingSet<C>,
    ) -> anyhow::Result<()> {
        if self
            .allowed_sequencers
            .get(da_address, working_set)
            .is_some()
        {
            anyhow::bail!("sequencer {} already registered", rollup_address)
        }
        let locker = &self.address;
        let coins = self.coins_to_lock.get_or_err(working_set)?;
        self.bank
            .transfer_from(rollup_address, locker, coins, working_set)?;

        self.allowed_sequencers
            .set(da_address, rollup_address, working_set);

        Ok(())
    }

    /// Returns the preferred sequencer, or [`None`] it wasn't set.
    ///
    /// Read about [`SequencerConfig::is_preferred_sequencer`] to learn about
    /// preferred sequencers.
    pub fn get_preferred_sequencer(&self, working_set: &mut WorkingSet<C>) -> Option<Da::Address> {
        self.preferred_sequencer.get(working_set)
    }

    /// Returns the rollup address of the preferred sequencer, or [`None`] it wasn't set.
    ///
    /// Read about [`SequencerConfig::is_preferred_sequencer`] to learn about
    /// preferred sequencers.
    pub fn get_preferred_sequencer_rollup_address(
        &self,
        working_set: &mut WorkingSet<C>,
    ) -> Option<C::Address> {
        self.preferred_sequencer.get(working_set).map(|da_addr| {
            self.allowed_sequencers
                .get(&da_addr, working_set)
                .expect("Preferred Sequencer must have known address on rollup")
        })
    }

    /// Checks whether `sender` is a registered sequencer.
    pub fn is_sender_allowed(&self, sender: &Da::Address, working_set: &mut WorkingSet<C>) -> bool {
        self.allowed_sequencers.get(sender, working_set).is_some()
    }
}