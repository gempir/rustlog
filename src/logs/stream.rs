use crate::{error::Error, Result};
use clickhouse::query::RowCursor;
use futures::{
    stream::{self, Iter},
    Future, Stream,
};
use std::{
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
    vec::IntoIter,
};
use tokio::pin;

pub enum LogsStream {
    Cursor {
        cursor: RowCursor<String>,
        first_item: Option<String>,
    },
    Provided(Iter<IntoIter<String>>),
}

impl LogsStream {
    pub async fn new_cursor(mut cursor: RowCursor<String>) -> Result<Self> {
        // Prefetch the first row to check that the response is not empty
        let first_item = cursor.next().await?.ok_or_else(|| Error::NotFound)?;
        Ok(Self::Cursor {
            cursor,
            first_item: Some(first_item),
        })
    }

    pub fn new_provided(iter: Vec<String>) -> Result<Self> {
        if iter.is_empty() {
            Err(Error::NotFound)
        } else {
            Ok(Self::Provided(stream::iter(iter)))
        }
    }
}

impl Stream for LogsStream {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.deref_mut() {
            LogsStream::Cursor { cursor, first_item } => {
                if let Some(item) = first_item.take() {
                    Poll::Ready(Some(Ok(item)))
                } else {
                    let fut = cursor.next();
                    pin!(fut);
                    fut.poll(cx)
                        .map(|result| result.map_err(|err| err.into()).transpose())
                }
            }
            LogsStream::Provided(iter) => Pin::new(iter).poll_next(cx).map(|item| item.map(Ok)),
        }
    }
}
