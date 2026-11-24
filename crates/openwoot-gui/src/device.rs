use std::{pin::Pin, thread, time::Duration};

use async_channel::{Receiver, Sender};
use async_io::Timer;
use futures_lite::future;
use openwoot_host::{
    Connection, channel,
    flash::{self, Event, Plan},
    hid::Hid,
    keyboard, transfer,
};
use openwoot_protocol::{Endpoint, Version, configuration::Rejection};
use serde::{Serialize, de::DeserializeOwned};

const RECONNECT_INTERVAL: Duration = Duration::from_secs(1);

type Job = Box<dyn for<'link> FnOnce(&'link mut Link) -> Pending<'link> + Send>;
type Pending<'link> = Pin<Box<dyn Future<Output = ()> + 'link>>;

#[derive(Clone)]
pub(crate) struct Device {
    jobs: Sender<Job>,
}

#[derive(Debug, Clone)]
pub(crate) enum Presence {
    Connected { name: String, version: Version },
    Absent(Option<String>),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failure {
    #[error("no keyboard is connected")]
    NotConnected,
    #[error("the keyboard worker has stopped")]
    Stopped,
    #[error(transparent)]
    Channel(#[from] channel::Error),
    #[error(transparent)]
    Rejected(#[from] Rejection),
    #[error(transparent)]
    Flash(#[from] flash::Error),
}

pub(crate) struct Link {
    hid: Hid,
    connection: Option<Connection>,
    presence: Sender<Presence>,
    announced: Announced,
}

#[derive(Debug, PartialEq, Eq)]
enum Announced {
    Nothing,
    Absent(Option<String>),
}

impl Device {
    pub(crate) fn start() -> Result<(Self, Receiver<Presence>), Failure> {
        let (jobs, pending) = async_channel::unbounded::<Job>();
        let (presence, presence_updates) = async_channel::unbounded();
        thread::Builder::new()
            .name("keyboard".to_owned())
            .spawn(move || async_io::block_on(serve(pending, presence)))
            .map_err(|_| Failure::Stopped)?;
        Ok((Self { jobs }, presence_updates))
    }

    pub(crate) fn call<E>(
        &self,
        request: E::Request,
    ) -> impl Future<Output = Result<E::Response, Failure>> + 'static
    where
        E: Endpoint,
        E::Request: Serialize + Send + 'static,
        E::Response: DeserializeOwned + Send + 'static,
    {
        let (reply, answer) = async_channel::bounded(1);
        let job: Job = Box::new(move |link: &mut Link| {
            Box::pin(async move {
                let result = link.request::<E>(&request).await;
                reply.try_send(result).ok();
            })
        });
        let queued = self.jobs.try_send(job);
        async move {
            queued.map_err(|_| Failure::Stopped)?;
            answer.recv().await.map_err(|_| Failure::Stopped)?
        }
    }

    pub(crate) fn configure<E, T>(
        &self,
        request: E::Request,
    ) -> impl Future<Output = Result<T, Failure>> + 'static
    where
        E: Endpoint<Response = Result<T, Rejection>>,
        E::Request: Serialize + Send + 'static,
        T: DeserializeOwned + Send + 'static,
    {
        let call = self.call::<E>(request);
        async move { Ok(call.await??) }
    }

    pub(crate) fn install(
        &self,
        plan: Plan,
        events: Sender<Event>,
    ) -> impl Future<Output = Result<(), Failure>> + 'static {
        let (reply, answer) = async_channel::bounded(1);
        let job: Job = Box::new(move |link: &mut Link| {
            Box::pin(async move {
                let result = link.install(&plan, &events).await;
                reply.try_send(result).ok();
            })
        });
        let queued = self.jobs.try_send(job);
        async move {
            queued.map_err(|_| Failure::Stopped)?;
            answer.recv().await.map_err(|_| Failure::Stopped)?
        }
    }
}

impl Link {
    async fn request<E>(&mut self, request: &E::Request) -> Result<E::Response, Failure>
    where
        E: Endpoint,
        E::Request: Serialize,
        E::Response: DeserializeOwned,
    {
        if self.connection.is_none() {
            self.connect().await;
        }
        let connection = self.connection.as_mut().ok_or(Failure::NotConnected)?;
        let answer = connection.request::<E>(request).await;
        if let Err(error) = &answer
            && is_lost(error)
        {
            self.lose(error.to_string());
        }
        Ok(answer?)
    }

    async fn install(&mut self, plan: &Plan, events: &Sender<Event>) -> Result<(), Failure> {
        self.connection = None;
        self.announced = Announced::Nothing;
        let installed = flash::install(&self.hid, plan, |event| {
            events.try_send(event).ok();
        })
        .await;
        self.connect().await;
        Ok(installed?)
    }

    async fn connect(&mut self) {
        match Connection::open(&self.hid).await {
            Ok(connection) => {
                let presence = Presence::Connected {
                    name: connection.info().name.clone(),
                    version: connection.version(),
                };
                self.connection = Some(connection);
                self.announced = Announced::Nothing;
                self.presence.try_send(presence).ok();
            }
            Err(error) => {
                let reason = match error {
                    keyboard::Error::NotFound => None,
                    error => Some(format!("{error:#}")),
                };
                self.announce_absence(reason);
            }
        }
    }

    fn lose(&mut self, reason: String) {
        self.connection = None;
        self.announce_absence(Some(reason));
    }

    fn announce_absence(&mut self, reason: Option<String>) {
        if self.announced != Announced::Absent(reason.clone()) {
            self.announced = Announced::Absent(reason.clone());
            self.presence.try_send(Presence::Absent(reason)).ok();
        }
    }
}

async fn serve(jobs: Receiver<Job>, presence: Sender<Presence>) {
    let mut link = Link {
        hid: Hid::default(),
        connection: None,
        presence,
        announced: Announced::Nothing,
    };
    loop {
        if link.connection.is_none() {
            link.connect().await;
        }
        let next = future::or(async { jobs.recv().await.map(Some) }, async {
            Timer::after(RECONNECT_INTERVAL).await;
            Ok(None)
        })
        .await;
        match next {
            Ok(Some(job)) => job(&mut link).await,
            Ok(None) => {}
            Err(_) => return,
        }
    }
}

fn is_lost(error: &channel::Error) -> bool {
    matches!(
        error,
        channel::Error::Transfer(transfer::Error::Disconnected | transfer::Error::Failed(_))
            | channel::Error::NoReply
    )
}
