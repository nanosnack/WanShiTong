{ self, pkgs }:
pkgs.testers.runNixOSTest {
  name = "wanshitong";

  nodes.machine = {
    imports = [ self.nixosModules.default ];

    services.wanshitong = {
      enable = true;
      libraryPaths = [ "/srv/media" ];
      dataSources.Movies.rootPath = "/srv/movies";
      viewPaths = [ "/srv/media/views" ];
      viewGroup = "media";
      settings.watch_quiet_period_secs = 1;
    };

    users.groups.media = { };
    systemd.tmpfiles.rules = [
      "d /srv/media 0755 root root -"
      "d /srv/media/views 2775 root media -"
      "d /srv/movies 0755 root root -"
    ];
  };

  testScript = ''
    machine.wait_for_unit("wanshitong.service")
    machine.wait_for_open_port(6765)
    machine.succeed("curl --fail http://127.0.0.1:6765/")
    assert '"status":"ok"' in machine.succeed("curl --fail http://127.0.0.1:6765/api/v1/health")
    machine.succeed("test -f /var/lib/wanshitong/db.sqlite")

    # The library is visible inside the service's mount namespace, read-only
    pid = machine.succeed("systemctl show --property=MainPID --value wanshitong").strip()
    ns = f"${pkgs.util-linux}/bin/nsenter --target {pid} --mount"
    machine.succeed(f"{ns} ${pkgs.coreutils}/bin/ls /srv/media")
    machine.fail(f"{ns} ${pkgs.coreutils}/bin/touch /srv/media/x")
    machine.succeed("${pkgs.coreutils}/bin/touch /srv/media/y")

    # Views are writable inside the sandbox, even inside a read-only library
    machine.succeed(f"{ns} ${pkgs.coreutils}/bin/touch /srv/media/views/x")
    machine.fail(f"{ns} ${pkgs.coreutils}/bin/touch /srv/media/z")

    # Declared data sources are created once, even across restarts
    sources = "${pkgs.sqlite}/bin/sqlite3 /var/lib/wanshitong/db.sqlite 'SELECT name, root_path FROM data_sources JOIN fs_data_sources USING (id)'"
    assert machine.succeed(sources).strip() == "Movies|/srv/movies"
    machine.succeed(f"{ns} ${pkgs.coreutils}/bin/ls /srv/movies")
    machine.systemctl("restart wanshitong")
    machine.wait_for_open_port(6765)
    assert machine.succeed(sources).strip() == "Movies|/srv/movies"

    # A file added to a data source from outside the sandbox gets ingested
    machine.succeed("echo x > /srv/movies/new.txt")
    recorded = "${pkgs.sqlite}/bin/sqlite3 /var/lib/wanshitong/db.sqlite \"SELECT count(*) FROM file_measurements WHERE rel_path = 'new.txt'\""
    machine.wait_until_succeeds(f"test \"$({recorded})\" -gt 0")
  '';
}
