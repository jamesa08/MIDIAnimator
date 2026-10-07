#!/bin/bash
# collects info about the Blender bridge connection on macOS
# run with MotionKeys and Blender both open: bash bridge_diagnostics.sh
# the report is saved to the Desktop and copied to the clipboard

report="$HOME/Desktop/motionkeys_bridge_report.txt"
support="$HOME/Library/Application Support"
settings="$support/com.jamesa08.midianimator/settings.json"

# the home folder becomes ~, other user folders /Users/<user>, the user name on its own <user> (whole words only, so com.jamesa08 stays)
anonymize() {
    sed -e "s|$HOME|~|g" -e "s|/Users/[^/ ]*|/Users/<user>|g" -e "s/[[:<:]]$USER[[:>:]]/<user>/g"
}

{
    echo "== system"
    sw_vers
    echo "arch: $(uname -m)"
    date

    # every copy of the app, with version and quarantine flag
    echo
    echo "== motionkeys installs"
    apps=$(mdfind "kMDItemCFBundleIdentifier == 'com.jamesa08.midianimator'" 2>/dev/null)
    [ -n "$apps" ] || echo "none found"
    echo "$apps" | while read -r app; do
        [ -n "$app" ] || continue
        version=$(defaults read "$app/Contents/Info" CFBundleShortVersionString 2>/dev/null)
        quarantine=$(xattr -p com.apple.quarantine "$app" 2>/dev/null)
        echo "$app (version ${version:-?}) quarantine: ${quarantine:-none}"
    done

    echo
    echo "== running processes"
    ps -axo pid,lstart,comm | grep -i -E "motionkeys|midianimator|/MacOS/Blender$" | grep -v grep

    # port from settings, MotionKeys uses 6577 unless it's a number from 1024 to 65535 (a quoted "6577" doesn't count)
    echo
    echo "== settings"
    port=6577
    if [ -f "$settings" ]; then
        saved=$(plutil -extract ipc.port raw -o - "$settings" 2>/dev/null)
        type=$(plutil -type ipc.port "$settings" 2>/dev/null)
        echo "settings file found, ipc.port: ${saved:-unset} (${type:-unknown type})"
        if [ "$type" = "integer" ] && [ "$saved" -ge 1024 ] && [ "$saved" -le 65535 ]; then
            port=$saved
        elif [ -n "$saved" ]; then
            echo "not a valid port, MotionKeys ignores it"
        fi
    else
        echo "no settings file at $settings"
    fi
    echo "port MotionKeys uses: $port"
    ports=$(printf "%s\n6577\n" "$port" | sort -u)

    # who holds the port, MotionKeys should be LISTEN and Blender ESTABLISHED
    for check_port in $ports; do
        echo
        echo "== port $check_port"
        lsof -nP -iTCP:"$check_port" 2>/dev/null || echo "nothing is using port $check_port"
    done

    # the bridge port and the MCP port, a changed port setting only applies after a restart
    echo
    echo "== motionkeys listening sockets"
    listening=$(lsof -nP -a -iTCP -sTCP:LISTEN -c MotionKeys -c MIDIAnim 2>/dev/null)
    echo "${listening:-none}"

    echo
    echo "== localhost"
    grep -i localhost /etc/hosts
    dscacheutil -q host -a name localhost

    echo
    echo "== blender installs"
    mdfind "kMDItemCFBundleIdentifier == 'org.blenderfoundation.blender'" 2>/dev/null | while read -r app; do
        version=$(defaults read "$app/Contents/Info" CFBundleShortVersionString 2>/dev/null)
        echo "$app (version ${version:-?})"
    done

    # every bridge add-on copy, and whether its folder sits where Blender loads it from
    echo
    echo "== bridge add-on installs"
    # one folder deeper than Blender looks too, a zip unpacked into an extra folder never loads
    installs=$(grep -l -E "MotionKeys Bridge|MIDIAnimator Bridge" "$support"/Blender/*/scripts/addons/*/__init__.py "$support"/Blender/*/scripts/addons/*/*/__init__.py "$support"/Blender/*/extensions/*/*/__init__.py 2>/dev/null)
    [ -n "$installs" ] || echo "none found"
    modules="motionkeys_bridge|midianimator_bridge"
    while read -r init; do
        [ -n "$init" ] || continue
        folder=$(dirname "$init")
        modules="$modules|$(basename "$folder")"
        echo "$folder"
        echo "  $(grep -o '"name": *"[^"]*"' "$init")"
        parent=$(basename "$(dirname "$folder")")
        echo "  parent: $parent"
        case "$parent" in
            addons | user_default | blender_org) ;;
            *) echo "  nested one folder too deep, Blender won't load this copy" ;;
        esac
        [ -L "$folder" ] && echo "  symlink to $(readlink "$folder")"
        for file in src/core.py src/tracker.py ui/__init__.py; do
            [ -f "$folder/$file" ] || echo "  missing $file"
        done
        grep -n "self.host = \|self.port = " "$folder/src/core.py" 2>/dev/null | sed 's/^/  /'
    done <<< "$installs"

    # enabled add-ons are saved by module name in each version's preferences
    echo
    echo "== add-on enabled in preferences"
    for prefs in "$support"/Blender/*/config/userpref.blend; do
        [ -f "$prefs" ] || continue
        version=$(basename "$(dirname "$(dirname "$prefs")")")
        found=$(grep -a -o -E "$modules" "$prefs" | sort -u | tr '\n' ' ')
        echo "$version: ${found:-not found}"
    done

    # connect the same way the add-on does, skipped while Blender is connected so the live link isn't dropped
    echo
    echo "== connection test"
    if lsof -nP -a -iTCP -sTCP:ESTABLISHED -c Blender 2>/dev/null | grep -q -E "127\.0\.0\.1:($(echo $ports | tr ' ' '|')) "; then
        echo "skipped, Blender is already connected"
    else
        # the running Blender's python, else any installed one
        blender=$(ps -axo comm | grep -m1 "/Contents/MacOS/Blender$")
        python=""
        for candidate in "${blender%/MacOS/Blender}"/Resources/*/python/bin/python3* /Applications/Blender*.app/Contents/Resources/*/python/bin/python3*; do
            case "$candidate" in *-config) continue ;; esac
            if [ -x "$candidate" ]; then
                python="$candidate"
                break
            fi
        done
        for check_port in $ports; do
            echo "-- port $check_port"
            if [ -n "$python" ]; then
                "$python" -I -c '
import socket, sys
port = int(sys.argv[1])
try:
    print("localhost resolves to", sorted({info[4][0] for info in socket.getaddrinfo("localhost", port, socket.AF_INET)}))
except Exception as e:
    print("resolve failed:", repr(e))
s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
s.settimeout(3)
try:
    s.connect(("localhost", port))
    print("connect ok")
except Exception as e:
    print("connect failed:", repr(e))
finally:
    s.close()
' "$check_port"
            else
                nc -4 -z -G 3 localhost "$check_port" && echo "connect ok" || echo "connect failed"
            fi
        done
        [ -n "$python" ] && echo "tested with $python"
    fi

    echo
    echo "== firewall"
    /usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate 2>/dev/null
    /usr/libexec/ApplicationFirewall/socketfilterfw --getstealthmode 2>/dev/null

    # the app's own log: which build started, the bridge listening or failing to, Blender connecting, panics
    echo
    echo "== motionkeys log"
    log_file="$HOME/Library/Logs/com.jamesa08.midianimator/motionkeys.log"
    if [ -f "$log_file" ]; then
        tail -n 200 "$log_file"
    else
        echo "no log at $log_file"
    fi
} 2>&1 | anonymize | tee "$report"

pbcopy < "$report"
echo
echo "saved to ~/Desktop/motionkeys_bridge_report.txt and copied to the clipboard"
