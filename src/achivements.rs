use anyhow::{Context, Result};
use rusqlite::{named_params, Connection};

pub fn check_achivement_status(db_filename: &str) -> Result<()> {
    let conn = Connection::open(db_filename)?;

    check_industry_standard(&conn)?;
    check_like_a_farmer(&conn)?;

    Ok(())
}

fn print_boxed(s: &str) {
    println!("╭─{}─╮", "─".repeat(s.len()));
    println!("│ {} │", s);
    println!("╰─{}─╯", "─".repeat(s.len()));
}

fn check_recipient_count(
    conn: &Connection,
    achivement_name: &str,
    recipients: &[&str],
    min_count: usize,
) -> Result<()> {
    conn.execute_batch(
        "
        DROP VIEW IF EXISTS temp.v_deliveries;
        CREATE TEMP VIEW IF NOT EXISTS v_deliveries (sender, recipient) AS
        SELECT params->>1 AS sender,
               params->>2 AS recipient
          FROM delivery_log_entry;
    ",
    )
    .context("failed to create v_deliveries view")?;

    print_boxed(achivement_name);

    for r in recipients {
        let completed: usize = conn
            .query_row(
                "
            SELECT COUNT(*) AS completed
              FROM temp.v_deliveries
             WHERE recipient = :r
        ",
                named_params! { ":r": r },
                |row| row.get("completed"),
            )
            .context("failed to query for completed deliveries")?;

        let prefix = if completed < min_count {
            "  "
        } else {
            "\x1b[1;32m✓\x1b[0m "
        };

        println!("{} {}/{}: {}", prefix, completed, min_count, r);
    }

    println!();

    Ok(())
}

fn check_industry_standard(conn: &Connection) -> Result<()> {
    check_recipient_count(
        conn,
        "Industry Standard",
        vec![
            "company.volatile.renat.tartu",
            "company.volatile.renat.helsinki",
            "company.volatile.renat.daugavpils",
            "company.volatile.renat.rezekne",
            "company.volatile.renat.riga",
            "company.volatile.renat.siauliai",
            "company.volatile.ee_paper.kunda",
            "company.volatile.viljo_paper.kouvola",
            "company.volatile.viljo_paper.tampere",
            "company.volatile.viln_paper.vilnius",
            "company.volatile.lvr.daugavpils",
            "company.volatile.lvr.riga",
        ]
        .as_ref(),
        2,
    )?;
    Ok(())
}

fn check_like_a_farmer(conn: &Connection) -> Result<()> {
    check_recipient_count(
        conn,
        "Like a Farmer",
        vec![
            "company.volatile.onnelik.narva",
            "company.volatile.onnelik.parnu",
            "company.volatile.onnelik_a.parnu",
            "company.volatile.onnelik_a.tartu",
            "company.volatile.egres.helsinki",
            "company.volatile.egres.kouvola",
            "company.volatile.eviksi.daugavpils",
            "company.volatile.eviksi.liepaja",
            "company.volatile.eviksi_a.liepaja",
            "company.volatile.eviksi.riga",
            "company.volatile.eviksi_a.valmiera",
            "company.volatile.eviksi_a.ventspils",
            "company.volatile.agrominta.utena",
            "company.volatile.agrominta_a.utena",
            "company.volatile.zelenye_a.kaliningrad",
            "company.volatile.zelenye.petersburg",
        ]
        .as_ref(),
        1,
    )?;
    Ok(())
}
