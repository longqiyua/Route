$path = "C:\Users\longq\Desktop\route (1)\crates\route-tauri\web\src\App.tsx"
$content = Get-Content $path -Raw

# Find start: <div className="app"> (top-level app wrapper, not the empty-state)
$needle = '      <div className="app">'
$i1 = $content.IndexOf($needle)
if ($i1 -lt 0) { Write-Output "START NOT FOUND"; exit 1 }

# Find the end of the main return: closing "    </div>\n  );\n}" - the last return ends before function declaration
# Look for the line that ends the main return block.
# The structure is: return ( <div className="app-shell"> <Titlebar/> <div className="app"> ... </div> </div> );
# End marker: pattern that closes the outer JSX.
$endMarker = '      </main>' + "`n    </div>`n    </div>`n  );"
$i2 = $content.IndexOf($endMarker)
if ($i2 -lt 0) {
    # try alternate: just </main></div></div>);
    $endMarker = "      </main>`n    </div>`n    </div>`n  );"
    $i2 = $content.IndexOf($endMarker)
}
if ($i2 -lt 0) {
    Write-Output "END NOT FOUND. Last 600 chars:"
    Write-Output $content.Substring($content.Length - 600)
    exit 1
}

$endIdx = $i2 + $endMarker.Length
$head = $content.Substring(0, $i1)
$tail = $content.Substring($endIdx)

$middle = @'
      <div className="app">
        <Sidebar
          projects={projects}
          activeProjectId={activeProjectId}
          onSelectProject={setActiveProjectId}
          onAddProject={handlePickFolder}
          onRemoveProject={removeProject}
          appOn={appOn}
          onToggleApp={() => setAppOn((v) => !v)}
        />

        <main className="main">
          {error && <div className="error-banner">{error}</div>}

          <div className="page-rail">
            <button
              className={page === "workspace" ? "active" : ""}
              onClick={() => setPage("workspace")}
              title="工作台"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="2" width="5" height="5" rx="1" />
                  <rect x="9" y="2" width="5" height="5" rx="1" />
                  <rect x="2" y="9" width="5" height="5" rx="1" />
                  <rect x="9" y="9" width="5" height="5" rx="1" />
                </svg>
              </span>
              <span>工作台</span>
            </button>
            <button
              className={page === "settings" ? "active" : ""}
              onClick={() => setPage("settings")}
              title="设置"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="8" cy="8" r="2" />
                  <path d="M8 1.5v1.8M8 12.7v1.8M14.5 8h-1.8M3.3 8H1.5M12.6 3.4 11.3 4.7M4.7 11.3 3.4 12.6M12.6 12.6 11.3 11.3M4.7 4.7 3.4 3.4" />
                </svg>
              </span>
              <span>设置</span>
            </button>
            <button
              className={page === "history" ? "active" : ""}
              onClick={() => setPage("history")}
              title="历史"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="8" cy="8" r="6" />
                  <path d="M8 4v4l2.5 1.5" />
                </svg>
              </span>
              <span>历史</span>
            </button>
          </div>

          {page === "workspace" && (
            mode === "basic" ? (
              <BasicMode
                endpoint={endpoint}
                onEndpointChange={setEndpoint}
                routeaEnabled={routeaEnabled}
                onToggleRoutea={toggleRoutea}
                appOn={appOn}
                onError={setError}
                loading={loading}
              />
            ) : (
              <AILockedOverlay onBack={() => setMode("basic")} />
            )
          )}

          {page === "settings" && (
            <SettingsPage
              project={activeProject}
              endpoint={endpoint}
              onEndpointChange={setEndpoint}
              routeaEnabled={routeaEnabled}
              onToggleRoutea={toggleRoutea}
              locale={locale}
              onLocaleChange={setLocale}
            />
          )}

          {page === "history" && (
            <HistoryPage commits={commits} projectName={activeProject?.name} />
          )}
        </main>
      </div>
'@

$new = $head + $middle + "`n" + $tail
Set-Content -Path $path -Value $new -NoNewline
Write-Output "Rewrote main UI. Length: $($new.Length)"
