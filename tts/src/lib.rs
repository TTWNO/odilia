use ssip_client::{
	fifo::asynchronous_tokio::Builder,
	tokio::{AsyncClient, Request},
	ClientName,
};
use std::{
	io::ErrorKind,
	process::{exit, Command, Stdio},
	thread, time,
};
use tokio::{
	io::{BufReader, BufWriter},
	net::unix::{OwnedReadHalf, OwnedWriteHalf},
	sync::{broadcast, mpsc::Receiver},
};

pub async fn create_ssip_client(
) -> eyre::Result<AsyncClient<BufReader<OwnedReadHalf>, BufWriter<OwnedWriteHalf>>> {
	tracing::debug!("Attempting to register SSIP client odilia:speech");
	let mut ssip_core =
		match Builder::new().build().await {
			Ok(ssip) => ssip,
			Err(e) => match e.kind() {
				ErrorKind::ConnectionRefused => {
					tracing::debug!("Speech dispatcher is not active. Attempting to spawn it.");
					Command::new("speech-dispatcher")
              .arg("--spawn")
              .stdin(Stdio::null())
              .stdout(Stdio::null())
              .stderr(Stdio::null())
              .spawn()
              .expect("Error running `speech-dispatcher --spawn`; this is a fatal error.");
					tracing::debug!(
						"Attempting to connect to speech-dispatcher again!"
					);
					thread::sleep(time::Duration::from_millis(500));
					Builder::new().build().await?
				}
				_ => {
					tracing::debug!("Speech dispatcher could not be started.");
					exit(1);
				}
			},
		};
	tracing::debug!("Client created. Setting name");
	ssip_core
		.set_client_name(ClientName::new("odilia", "speech"))
		.await?
		.check_client_name_set()
		.await?;
	tracing::debug!("SSIP client registered as odilia:speech");
	Ok(ssip_core)
}

pub async fn handle_ssip_commands(
	client: &mut AsyncClient<BufReader<OwnedReadHalf>, BufWriter<OwnedWriteHalf>>,
	requests: Receiver<Request>,
	shutdown_tx: &mut broadcast::Receiver<i32>,
) -> eyre::Result<()> {
	tokio::pin!(requests);
	loop {
		tokio::select! {
				request_option = requests.recv() => {
					if request_option.is_none() {
						continue;
					}
					let request = request_option.unwrap();
					tracing::debug!("SSIP command received");
		let response = client
		  .send(request).await.unwrap()
		  .receive().await.unwrap();
		tracing::debug!("Response from server: {:#?}", response);
					continue;
				}
				_ = shutdown_tx.recv() => {
		tracing::debug!("Saying goodbye message.");
		let _ = client
			.send(Request::Speak).await.unwrap()
			.receive().await.unwrap();
		let _ = client
			.send(Request::SendLines(Vec::from(["Quitting Odilia".to_string()]))).await.unwrap()
			.receive().await.unwrap();
		tracing::debug!("Attempting to quit SSIP.");
		let response = client
		  .send(Request::Quit).await.unwrap()
		  .receive().await.unwrap();
		tracing::debug!("Response from server: {:?}", response);
					tracing::debug!("SSIP command interpreter is shut down.");
					break;
				}
			}
	}
	Ok(())
}
