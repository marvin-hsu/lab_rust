use byteorder::{BigEndian, ByteOrder};
use sqlx::{Decode, Encode, Postgres, Type};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueFormat, PgValueRef};
use sqlx::postgres::types::Oid;


// #[derive(sqlx::Type)]
// #[sqlx(type_name = "xid")]
// pub struct Xid(pub i32);

pub struct Xid(pub u32);

impl Type<Postgres> for Xid {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_oid(Oid(28))
    }
}

impl Encode<'_, Postgres> for Xid {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> IsNull {
        buf.extend(&self.0.to_be_bytes());

        IsNull::No
    }
}

impl Decode<'_, Postgres> for Xid {
    fn decode(value: PgValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(Self(match value.format() {
            PgValueFormat::Binary => BigEndian::read_u32(value.as_bytes()?),
            PgValueFormat::Text => value.as_str()?.parse()?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{Connection, postgres::PgConnection, types::Uuid};
    use testcontainers::{RunnableImage, runners::AsyncRunner};
    use testcontainers_modules::postgres::Postgres;

    use crate::Xid;

    #[tokio::test]
    async fn it_works() {
        let container = RunnableImage::from(
            Postgres::default()
                .with_db_name("test")
                .with_user("test")
                .with_password("1234"),
        )
            .with_tag("16.2-bullseye")
            .start()
            .await;

        let connection_string = format!(
            "postgres://test:1234@127.0.0.1:{}/test",
            container.get_host_port_ipv4(5432).await
        );

        let mut conn = PgConnection::connect(connection_string.as_str())
            .await
            .expect("Connect to Postgres Fail.");

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS test_table (
                id UUID PRIMARY KEY,
                value VARCHAR
            )
            "#,
        )
            .execute(&mut conn)
            .await
            .expect("Create table fail.");

        let id = Uuid::new_v4();

        // insert a row into the table
        sqlx::query(
            r#"
            INSERT INTO test_table (id, value) VALUES ($1, $2)
            "#,
        )
            .bind(id)
            .bind("test")
            .execute(&mut conn)
            .await
            .expect("Insert row fail.");

        let row: (Xid, ) = sqlx::query_as(
            r#"
            SELECT xmin FROM test_table WHERE id = $1
            "#,
        )
            .bind(id)
            .fetch_one(&mut conn)
            .await
            .expect("Select row fail.");

        let new: (Uuid, ) = sqlx::query_as(
            r#"
            SELECT id FROM test_table WHERE xmin = $1
            "#,
        )
            .bind(row.0)
            .fetch_one(&mut conn)
            .await
            .expect("Select row fail.");

        assert_eq!(new.0, id);
    }
}

