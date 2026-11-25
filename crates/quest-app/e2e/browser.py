#!/usr/bin/env -S python3 -I
"""Stand-in for the system browser in the E2E run (`quest-app --browser e2e/browser.py`).

Gets one URL, follows redirects with a fresh cookie jar like a browser would, and at the OIDC
fake's /authorize adds `login_hint=$QUEST_E2E_USER`, so the fake signs that user in without a
page. It stops at the first page that is no redirect: for a sign-in that is the app's own loopback
listener at http://127.0.0.1:<port>/callback, for a sign-out the server's "Signed out" page.
Writes one line per step to stderr (the app's log)."""

import http.cookiejar
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

MAX_REDIRECTS = 15


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def with_login_hint(url, idp_origin, user):
    parts = urllib.parse.urlsplit(url)
    origin = f"{parts.scheme}://{parts.netloc}"
    if origin != idp_origin or not parts.path.endswith("/authorize"):
        return url
    query = urllib.parse.parse_qsl(parts.query) + [("login_hint", user)]
    return urllib.parse.urlunsplit(parts._replace(query=urllib.parse.urlencode(query)))


def main():
    url = sys.argv[1]
    idp_origin = os.environ["QUEST_E2E_IDP"].rstrip("/")
    user = os.environ.get("QUEST_E2E_USER", "anna")
    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar), NoRedirect())
    for _ in range(MAX_REDIRECTS):
        url = with_login_hint(url, idp_origin, user)
        try:
            response = opener.open(url, timeout=10)
            print(f"browser: {response.status} {url.split('?')[0]}", file=sys.stderr)
            return 0
        except urllib.error.HTTPError as error:
            location = error.headers.get("Location")
            if error.code not in (301, 302, 303, 307, 308) or not location:
                print(f"browser: {error.code} {url.split('?')[0]}", file=sys.stderr)
                return 1
            url = urllib.parse.urljoin(url, location)
    print("browser: too many redirects", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
