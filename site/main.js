// Picks the download for the visitor's system. Runs only in the browser; nothing is sent
// anywhere. Without JavaScript the big button simply jumps to the download section.
(function () {
  var ua = navigator.userAgent || "";
  var platform = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  var s = platform + " " + ua;
  var os = null;
  if (/Android|iPhone|iPad|iPod/i.test(ua)) os = "mobile";
  else if (/Win/i.test(s)) os = "windows";
  else if (/Mac/i.test(s)) os = "macos";
  else if (/Linux|X11|CrOS/i.test(s)) os = "linux";

  var names = { windows: "Windows", macos: "macOS", linux: "Linux" };
  var button = document.getElementById("main-download");
  var note = document.getElementById("other-systems");
  if (!button) return;

  if (os === "mobile") {
    note.textContent = "Yaperino is a desktop app for Windows, macOS and Linux";
    return;
  }
  if (!names[os]) return;

  var tile = document.querySelector('.tile[data-os="' + os + '"]');
  var link = tile && tile.querySelector("a.button");
  if (link) {
    button.href = link.href;
    button.textContent = "Download for " + names[os];
    tile.classList.add("mine");
  }
  var others = Object.keys(names).filter(function (k) { return k !== os; });
  note.innerHTML = "";
  note.appendChild(document.createTextNode("Also for "));
  others.forEach(function (k, i) {
    var a = document.createElement("a");
    a.href = "#download";
    a.textContent = names[k];
    note.appendChild(a);
    if (i < others.length - 1) note.appendChild(document.createTextNode(" and "));
  });
})();
