import argparse
import subprocess
import json


from datetime import datetime


def main():
    parser = argparse.ArgumentParser(
        prog="register_token.py",
        description="Register eddiner token for local debugging",
    )
    parser.add_argument("-t", "--token", required=True)
    parser.add_argument("--db", required=True)
    args = parser.parse_args()

    def run_d1_command(sql):
        return subprocess.run(
            [
                "npx",
                "wrangler",
                "d1",
                "execute",
                "--local",
                "--json",
                args.db,
                "--command",
                sql,
            ],
            capture_output=True,
        )

    result = run_d1_command(
        f"SELECT * FROM authed_cookies WHERE cookie = '{args.token}'"
    )
    if result.returncode != 0:
        print(f"Command failed: {result.stderr}")
        exit(1)
    tables = json.loads(result.stdout)
    if len(tables[0]["results"]) > 0:
        ip = tables[0]["results"][0]["origin_ip"]
        print(f"Authenticating ip {ip}")
        unix_timestamp = (datetime.now() - datetime(1970, 1, 1)).total_seconds()
        update_sql = (
            f"UPDATE authed_cookies SET authed = 1, authed_time = '{unix_timestamp}'"
            + f" WHERE cookie = '{args.token}'"
        )
        result = run_d1_command(update_sql)
        if result.returncode != 0:
            print(f"Command failed: {result.stderr}")
            exit(1)
        else:
            print("Successfully registered token")
    else:
        print("Token not found")
        exit(1)


if __name__ == "__main__":
    main()
