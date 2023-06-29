// Copyright 2021-2022 System76 <info@system76.com>
// SPDX-License-Identifier: MPL-2.0

use as_result::*;
use async_stream::stream;
use futures::stream::Stream;
use std::{io, pin::Pin};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};

#[derive(AsMut, Deref, DerefMut)]
#[as_mut(forward)]
pub struct Dpkg(Command);

impl Dpkg {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut cmd = Command::new("dpkg");
        cmd.env("LANG", "C");
        Self(cmd)
    }

    pub fn force_confdef(mut self) -> Self {
        self.arg("--force-confdef");
        self
    }

    pub fn force_confold(mut self) -> Self {
        self.arg("--force-confold");
        self
    }

    pub fn configure_all(mut self) -> Self {
        self.args(["--configure", "-a"]);
        self
    }

    pub async fn status(mut self) -> io::Result<()> {
        self.0.status().await?.into_result()
    }
}

pub type InstalledEvent = Pin<Box<dyn Stream<Item = String>>>;

#[derive(AsMut, Deref, DerefMut)]
#[as_mut(forward)]
pub struct DpkgQuery(Command);

impl DpkgQuery {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let mut cmd = Command::new("dpkg-query");
        cmd.env("LANG", "C");
        Self(cmd)
    }

    pub async fn show_installed<I, S>(mut self, packages: I) -> io::Result<(Child, InstalledEvent)>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.args(["--show", "--showformat=${Package} ${db:Status-Status}\n"]);
        self.args(packages);

        let (child, stdout) = self.spawn_with_stdout().await?;

        let mut stdout = BufReader::new(stdout).lines();

        let stream = stream! {
            while let Ok(Some(line)) = stdout.next_line().await {
                let mut fields = line.split(' ');
                let package = fields.next().unwrap();
                if fields.next().unwrap() == "installed" {
                    yield package.into();
                }
            }
        };

        Ok((child, Box::pin(stream)))
    }

    pub async fn status(mut self) -> io::Result<()> {
        self.0.status().await?.into_result()
    }

    pub async fn spawn_with_stdout(self) -> io::Result<(Child, ChildStdout)> {
        crate::utils::spawn_with_stdout(self.0).await
    }
}
