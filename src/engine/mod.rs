use self::{
    error::EngineError,
    state::{
        Initialise,
        consume::Consume,
        market::UpdateFromMarket,
        order::{GenerateOrder, Algorithmic, Manual},
        account::UpdateFromAccount,
        command::ExecuteCommand,
        terminate::Terminate,
    }
};
use crate::{
    event::{Command, EventFeed},
    portfolio::{Initialiser, AccountUpdater, MarketUpdater},
    execution::ExecutionRequest,
};
use barter_integration::model::{Exchange, Instrument};
use barter_data::model::MarketEvent;
use barter_execution::model::AccountEvent;
use std::{
    collections::HashMap,
    marker::PhantomData
};
use tokio::sync::mpsc;


pub mod state;
pub mod error;

// Todo:
//  - Should AccountEvent contain an exchange_timestamp?
//  - May benefit from having 'EngineBuilder' build all components of the system
//   '--> ie/ spawns all threads & tasks for barter-data, execution, etc
//    --> "Engine" could become "TraderStates" or similar


pub enum Engine<Strategy, Portfolio>
where
    Portfolio: MarketUpdater + AccountUpdater
{
    Initialise(Trader<Strategy, Initialise<Portfolio>>),
    Consume(Trader<Strategy, Consume<Portfolio>>),
    UpdateFromMarket((Trader<Strategy, UpdateFromMarket<Portfolio>>, MarketEvent)),
    GenerateOrder(Trader<Strategy, GenerateOrder<Algorithmic>>),
    GenerateOrderManual((Trader<Strategy, GenerateOrder<Manual>>, ())),
    UpdateFromAccount((Trader<Strategy, UpdateFromAccount<Portfolio>>, AccountEvent)),
    ExecuteCommand((Trader<Strategy, ExecuteCommand<Portfolio>>, Command)),
    Terminate(Trader<Strategy, Terminate>)
}

pub struct Trader<Strategy, State> {
    pub feed: EventFeed,
    pub strategy: Strategy,
    pub execution_tx: mpsc::UnboundedSender<ExecutionRequest>,
    pub state: State,
}

impl<Strategy, Portfolio> Engine<Strategy, Portfolio>
where
    Portfolio: Initialiser<Output = Portfolio> + MarketUpdater + AccountUpdater
{
    /// Builder to construct [`Engine`] instances.
    pub fn builder() -> EngineBuilder<Strategy, Portfolio> {
        EngineBuilder::new()
    }

    pub fn run(mut self) {
        'trading: loop {
            // Transition to the next trading state
            self = self.next();

            if let Self::Terminate(_) = self {
                // Todo: Print trading session results & persist
                break 'trading
            }
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Initialise(trader) => {
                trader.init()
            }
            Self::Consume(trader) => {
                trader.next_event()
            },
            Self::UpdateFromMarket((trader, market)) => {
                trader.update(market)
            },
            Self::GenerateOrder(trader) => {
                todo!()
            }
            Self::GenerateOrderManual((trader, meta)) => {
                todo!()
            },
            Self::UpdateFromAccount((trader, account)) => {
                trader.update(account)
            }
            Self::ExecuteCommand((trader, command)) => {
                trader.execute_manual_command(command)
            }
            Self::Terminate(trader) => {
                Self::Terminate(trader)
            }
        }
    }
}

/// Builder to construct [`Engine`] instances.
#[derive(Default)]
pub struct EngineBuilder<Strategy, Portfolio> {
    pub feed: Option<EventFeed>,
    pub strategy: Option<Strategy>,
    pub execution_tx: Option<mpsc::UnboundedSender<ExecutionRequest>>,
    pub instruments: Option<HashMap<Exchange, Vec<Instrument>>>,
    pub phantom: PhantomData<Portfolio>,
}

impl<Strategy, Portfolio> EngineBuilder<Strategy, Portfolio>
where
    Portfolio: MarketUpdater + AccountUpdater,
{
    fn new() -> Self {
        Self {
            feed: None,
            strategy: None,
            execution_tx: None,
            instruments: None,
            phantom: PhantomData::default()
        }
    }

    pub fn feed(self, value: EventFeed) -> Self {
        Self {
            feed: Some(value),
            ..self
        }
    }

    pub fn strategy(self, value: Strategy) -> Self {
        Self {
            strategy: Some(value),
            ..self
        }
    }

    pub fn execution_tx(self, value: mpsc::UnboundedSender<ExecutionRequest>) -> Self {
        Self {
            execution_tx: Some(value),
            ..self
        }
    }

    pub fn instruments(self, value: HashMap<Exchange, Vec<Instrument>>) -> Self {
        Self {
            instruments: Some(value),
            ..self
        }
    }

    pub fn build(self) -> Result<Engine<Strategy, Portfolio>, EngineError> {
        Ok(Engine::Initialise(Trader {
            feed: self.feed.ok_or(EngineError::BuilderIncomplete("feed"))?,
            strategy: self.strategy.ok_or(EngineError::BuilderIncomplete("strategy"))?,
            execution_tx: self.execution_tx.ok_or(EngineError::BuilderIncomplete("execution_tx"))?,
            state: Initialise {
                instruments: self.instruments.ok_or(EngineError::BuilderIncomplete("instruments"))?,
                phantom: self.phantom
            }
        }))
    }
}