/* Daedric Toolkit — install doctor frontend.
 * Talks to the Rust command surface via window.__TAURI__.core.invoke
 * (app.withGlobalTauri: true). No bundler, no keyboard-only flows.
 */

(function () {
  "use strict";

  var invoke = window.__TAURI__.core.invoke;

  var $ = function (id) {
    return document.getElementById(id);
  };

  var asarInput = $("asar-path");
  var rootInput = $("skyrim-root");
  var runBtn = $("run-doctor");
  var statusLine = $("status-line");

  function dataDir() {
    return rootInput.value.replace(/\/+$/, "") + "/Data";
  }

  // ---------- verdict decoding (serde externally-tagged enums) ----------

  function verdictName(v) {
    if (typeof v === "string") return v;
    return Object.keys(v)[0];
  }

  function isOkVerdict(v) {
    var n = verdictName(v);
    return n === "Ok" || n === "Unpinned";
  }

  function shortHash(h) {
    if (!h) return "—";
    return h.length > 16 ? h.slice(0, 12) + "…" : h;
  }

  function verdictDetail(v) {
    var n = verdictName(v);
    if (n === "Missing") return "not found on disk";
    if (n === "Unpinned") return "no md5 pin in contract";
    if (typeof v !== "object") return "";
    var p = v[n];
    if (n === "HashMismatch") {
      var s = "expected sha " + shortHash(p.expected) + " / got " + shortHash(p.actual);
      if (p.actual_size !== undefined) s += " (size " + p.actual_size + ")";
      return s;
    }
    if (n === "SizeMismatch") {
      return "expected " + p.expected + " bytes / got " + p.actual;
    }
    return "";
  }

  // ---------- rendering ----------

  function setBadge(id, state) {
    // state: "idle" | "pass" | "fail"
    var b = $(id);
    b.className = "badge badge-" + state;
    b.textContent = state.toUpperCase();
  }

  function renderPanel(prefix, report, entries, nameOf, kindOf, verdictOf) {
    $("ok-" + prefix).textContent = report.ok + " ok";
    $("fail-" + prefix).textContent = report.failed + " failed";
    setBadge("badge-" + prefix, report.failed === 0 ? "pass" : "fail");

    var list = $("list-" + prefix);
    list.textContent = "";
    var failures = entries.filter(function (e) {
      return !isOkVerdict(verdictOf(e));
    });

    if (failures.length === 0) {
      var li = document.createElement("li");
      li.className = "empty-note";
      li.textContent = report.failed === 0 ? "the gate holds — no failures" : "no failing entries";
      list.appendChild(li);
      return;
    }

    failures.forEach(function (e) {
      var li = document.createElement("li");
      var head = document.createElement("div");
      var name = document.createElement("span");
      name.className = "fail-name";
      name.textContent = nameOf(e);
      head.appendChild(name);
      var kind = kindOf(e);
      if (kind) {
        var k = document.createElement("span");
        k.className = "fail-kind";
        k.textContent = "[" + kind + "]";
        head.appendChild(k);
      }
      var vn = document.createElement("span");
      vn.className = "fail-verdict";
      vn.textContent = verdictName(verdictOf(e));
      head.appendChild(vn);
      li.appendChild(head);

      var detail = verdictDetail(verdictOf(e));
      var extra = e.canonical_entry ? " · fix from: " + e.canonical_entry : "";
      if (detail || extra) {
        var d = document.createElement("div");
        d.className = "fail-detail";
        d.textContent = detail + extra;
        li.appendChild(d);
      }
      list.appendChild(li);
    });
  }

  function renderEsp(report) {
    renderPanel(
      "esp",
      report,
      report.files,
      function (f) { return f.esp; },
      function () { return null; },
      function (f) { return f.verdict; }
    );
  }

  function renderGate(report) {
    renderPanel(
      "gate",
      report,
      report.files,
      function (f) { return f.file; },
      function (f) { return f.kind; },
      function (f) { return f.verdict; }
    );
  }

  function renderDownloads(report) {
    renderPanel(
      "dl",
      report,
      report.archives,
      function (a) { return a.name; },
      function (a) { return a.category || null; },
      function (a) { return a.verdict; }
    );
  }

  function renderContracts(c) {
    $("contract-strip").classList.remove("hidden");
    $("c-mods").textContent = c.mods;
    $("c-pins").textContent = c.file_pins;
    $("c-plugins").textContent = c.plugins + " +" + c.loose_files + " loose";
    $("c-dlls").textContent = c.dlls;
    $("c-slug").textContent = c.slug || "—";
    $("c-rev").textContent = c.revision != null ? c.revision : "—";
    $("c-fileset").textContent = c.file_set_sha256
      ? c.file_set_sha256.slice(0, 24) + "…"
      : "—";
  }

  function panelError(prefix, err) {
    setBadge("badge-" + prefix, "fail");
    var list = $("list-" + prefix);
    list.textContent = "";
    var li = document.createElement("li");
    li.className = "empty-note";
    li.textContent = "scan failed: " + err;
    list.appendChild(li);
  }

  function setStatus(msg, isError) {
    statusLine.textContent = msg;
    statusLine.className = isError ? "status-line error" : "status-line";
  }

  function setBusy(busy) {
    runBtn.disabled = busy;
    ["scan-esp", "scan-gate", "scan-dl"].forEach(function (id) {
      $(id).disabled = busy;
    });
    runBtn.textContent = busy ? "CONSULTING THE GATE…" : "RUN FULL DOCTOR";
  }

  // ---------- actions ----------

  function runFullDoctor() {
    setBusy(true);
    setStatus("decoding contracts and scanning the install…", false);
    invoke("run_full_doctor", {
      skyrimRoot: rootInput.value,
      asarPath: asarInput.value,
    })
      .then(function (report) {
        renderContracts(report.contracts);
        renderEsp(report.esp);
        renderGate(report.gate);
        renderDownloads(report.downloads);
        var totalFail =
          report.esp.failed + report.gate.failed + report.downloads.failed;
        setStatus(
          totalFail === 0
            ? "the gate holds. every contract passes."
            : totalFail + " failure(s) across the three gates.",
          totalFail !== 0
        );
      })
      .catch(function (err) {
        setStatus("doctor failed: " + err, true);
      })
      .finally(function () {
        setBusy(false);
      });
  }

  function runSingle(btnId, prefix, cmd, args, render) {
    $(btnId).disabled = true;
    setBadge("badge-" + prefix, "idle");
    invoke(cmd, args)
      .then(render)
      .catch(function (err) {
        panelError(prefix, err);
      })
      .finally(function () {
        $(btnId).disabled = false;
      });
  }

  runBtn.addEventListener("click", runFullDoctor);

  $("scan-esp").addEventListener("click", function () {
    runSingle("scan-esp", "esp", "scan_esp_gate", {
      dataDir: dataDir(),
      asarPath: asarInput.value,
    }, renderEsp);
  });

  $("scan-gate").addEventListener("click", function () {
    runSingle("scan-gate", "gate", "scan_build_gate", {
      dataDir: dataDir(),
      asarPath: asarInput.value,
    }, renderGate);
  });

  $("scan-dl").addEventListener("click", function () {
    runSingle("scan-dl", "dl", "verify_downloads", {
      skyrimRoot: rootInput.value,
      asarPath: asarInput.value,
    }, renderDownloads);
  });

  // Decode the contract strip on load so the UI opens with context.
  invoke("decode_contracts", { asarPath: asarInput.value })
    .then(renderContracts)
    .catch(function () {
      /* asar may not exist on this machine — stay quiet, strip stays hidden */
    });
})();
