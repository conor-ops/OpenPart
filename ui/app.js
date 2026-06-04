(() => {
  const mapEl = document.getElementById("disk-map");
  const tableEl = document.getElementById("partition-table");
  const queueEl = document.getElementById("queue-list");
  const queueCountEl = document.getElementById("queue-count");
  const queueEmptyEl = document.getElementById("queue-empty");
  const startupLoadingEl = document.getElementById("startup-loading");
  const diskSelectEl = document.getElementById("disk-select");
  const diskStyleBadgeEl = document.getElementById("disk-style-badge");
  const diskSubtitleEl = document.getElementById("disk-subtitle");
  const diskStatusEl = document.getElementById("disk-status");
  const diskHealthEl = document.getElementById("disk-health");
  const modalEl = document.getElementById("resize-modal");
  const modalTitleEl = document.getElementById("resize-title");
  const resizeLeftEl = document.getElementById("resize-left");
  const resizeRightEl = document.getElementById("resize-right");
  const resizeShrinkLeftEl = document.getElementById("resize-shrink-left");
  const resizeShrinkRightEl = document.getElementById("resize-shrink-right");
  const resizeLeftValueEl = document.getElementById("resize-left-value");
  const resizeRightValueEl = document.getElementById("resize-right-value");
  const resizeShrinkLeftValueEl = document.getElementById("resize-shrink-left-value");
  const resizeShrinkRightValueEl = document.getElementById("resize-shrink-right-value");
  const resizeBeforeEl = document.getElementById("resize-before");
  const resizeAfterEl = document.getElementById("resize-after");
  const resizeChangesEl = document.getElementById("resize-changes");
  const resizeCurrentEl = document.getElementById("resize-current");
  const resizeMinEl = document.getElementById("resize-min");
  const resizeNewEl = document.getElementById("resize-new");
  const resizeApplyEl = document.getElementById("resize-apply");
  const resizeCloseEl = document.getElementById("resize-close");
  const resizeCancelEl = document.getElementById("resize-cancel");
  const moveModalEl = document.getElementById("move-modal");
  const moveTitleEl = document.getElementById("move-title");
  const moveSliderEl = document.getElementById("move-slider");
  const moveShiftValueEl = document.getElementById("move-shift-value");
  const moveBeforeSizeEl = document.getElementById("move-before-size");
  const moveAfterSizeEl = document.getElementById("move-after-size");
  const moveLayoutPreviewEl = document.getElementById("move-layout-preview");
  const moveBlockBeforeEl = document.getElementById("move-block-before");
  const moveBlockPartEl = document.getElementById("move-block-part");
  const moveBlockAfterEl = document.getElementById("move-block-after");
  const moveBlockBeforeSizeEl = document.getElementById("move-block-before-size");
  const moveBlockPartLabelEl = document.getElementById("move-block-part-label");
  const moveBlockPartSizeEl = document.getElementById("move-block-part-size");
  const moveBlockAfterSizeEl = document.getElementById("move-block-after-size");
  const moveCloseEl = document.getElementById("move-close");
  const moveCancelEl = document.getElementById("move-cancel");
  const moveApplyEl = document.getElementById("move-apply");
  const createModalEl = document.getElementById("create-modal");
  const createTargetEl = document.getElementById("create-target");
  const createFsEl = document.getElementById("create-fs");
  const createSizeRangeEl = document.getElementById("create-size-range");
  const createSizeInputEl = document.getElementById("create-size-input");
  const createLabelSelectEl = document.getElementById("create-label-select");
  const createLabelCustomEl = document.getElementById("create-label-custom");
  const createSpaceSizeEl = document.getElementById("create-space-size");
  const createSpaceCountEl = document.getElementById("create-space-count");
  const createCloseEl = document.getElementById("create-close");
  const createCancelEl = document.getElementById("create-cancel");
  const createQueueEl = document.getElementById("create-queue");
  const refreshBtn = document.getElementById("refresh-btn");
  const restoreBtn = document.getElementById("restore-btn");
  const settingsBtn = document.getElementById("settings-btn");
  const helpBtn = document.getElementById("help-btn");
  const infoBtn = document.getElementById("info-btn");
  const discardBtn = document.getElementById("discard-changes");
  const applyBtn = document.getElementById("apply-changes");
  const applyModalEl = document.getElementById("apply-modal");
  const applyCloseEl = document.getElementById("apply-close");
  const applyDismissEl = document.getElementById("apply-dismiss");
  const applyStatusEl = document.getElementById("apply-status");
  const applyProgressFillEl = document.getElementById("apply-progress-fill");
  const applyPercentEl = document.getElementById("apply-percent");
  const applyCountEl = document.getElementById("apply-count");
  const applyLogOpsEl = document.getElementById("panel-apply-ops");
  const applyLogWarningsEl = document.getElementById("panel-apply-warnings");
  const applyLogErrorsEl = document.getElementById("panel-apply-errors");
  const infoModalEl = document.getElementById("info-modal");
  const infoTitleEl = document.getElementById("info-title");
  const infoSubtitleEl = document.getElementById("info-subtitle");
  const infoBodyEl = document.getElementById("info-body");
  const infoCloseEl = document.getElementById("info-close");
  const infoCancelEl = document.getElementById("info-cancel");
  const infoDismissEl = document.getElementById("info-dismiss");
  const actionNewBtn = document.getElementById("action-new");
  const actionResizeBtn = document.getElementById("action-resize");
  const actionDeleteBtn = document.getElementById("action-delete");
  const actionFormatBtn = document.getElementById("action-format");
  const actionPropsBtn = document.getElementById("action-props");

  if (!mapEl || !tableEl || !queueEl || !queueCountEl || !queueEmptyEl) {
    console.warn("OpenPart UI: missing DOM nodes.");
    return;
  }

  const MIN_UNALLOCATED_GB = 0.001;
  const SAVED_QUEUE_KEY = "openpart.queue.v1";

  let state = { disks: [], queue: [] };
  let coreStateRaw = null;
  let selectedDiskIndex = 0;
  let dragState = null;
  let selectedId = null;
  let resizeState = null;
  let createState = null;
  let applyInProgress = false;
  let applyLogs = [];
  let applyProgressTimer = null;
  let applyProgressCurrent = 0;
  let applyProgressTotal = 0;
  let infoConfirmHandler = null;
  let dragRenderPending = false;

  const invoke =
    (window.__TAURI__ && window.__TAURI__.invoke) ||
    (window.__TAURI__ && window.__TAURI__.tauri && window.__TAURI__.tauri.invoke);

  if (invoke) {
    setStartupLoading(true);
    invoke("get_state")
      .then(async (coreState) => {
        coreStateRaw = coreState;
        const saved = getSavedQueue();
        if (saved.length > 0) {
          state.queue = saved;
          updateRestoreBadge();
          renderQueue();
          await updateComposedState();
        } else {
          state = mapFromCore(coreState);
          render();
          updateRestoreBadge();
        }
      })
      .catch((err) => {
        console.error("OpenPart failed to load state.", err);
        document.body.insertAdjacentHTML(
          "afterbegin",
          '<div style="position:fixed;left:12px;right:12px;top:12px;z-index:9999;padding:10px 14px;background:#2b1d1d;color:#ffd9d9;border:1px solid #7a3b3b;border-radius:10px;font:14px system-ui,sans-serif">Disk scan failed. Check the PowerShell backend and refresh.</div>'
        );
        render();
      })
      .finally(() => setStartupLoading(false));
  } else {
    // No Tauri backend available: do not present a browser UI.
    console.error("OpenPart requires the native Tauri backend.");
    document.body.innerHTML = "";
    return;
  }

  mapEl.addEventListener("pointerdown", onPointerDown);
  queueEl.addEventListener("click", onQueueClick);
  document.addEventListener("pointerup", onPointerUp);
  document.addEventListener("pointermove", onPointerMove);

  if (modalEl) {
    modalEl.addEventListener("click", (event) => {
      if (event.target === modalEl) {
        closeResizeModal();
      }
    });
  }

  if (createModalEl) {
    createModalEl.addEventListener("click", (event) => {
      if (event.target === createModalEl) {
        closeCreateModal();
      }
    });
  }

  if (applyModalEl) {
    applyModalEl.addEventListener("click", (event) => {
      if (event.target === applyModalEl && !applyInProgress) {
        closeApplyModal();
      }
    });
  }

  if (infoModalEl) {
    infoModalEl.addEventListener("click", (event) => {
      if (event.target === infoModalEl) {
        closeInfoModal();
      }
    });
  }

  if (resizeCloseEl) resizeCloseEl.addEventListener("click", closeResizeModal);
  if (resizeCancelEl) resizeCancelEl.addEventListener("click", closeResizeModal);
  if (resizeLeftEl) resizeLeftEl.addEventListener("input", () => {
    if (resizeShrinkLeftEl) resizeShrinkLeftEl.value = 0;
    if (resizeShrinkRightEl) resizeShrinkRightEl.value = 0;
    updateResizePreview();
  });
  if (resizeRightEl) resizeRightEl.addEventListener("input", () => {
    if (resizeShrinkLeftEl) resizeShrinkLeftEl.value = 0;
    if (resizeShrinkRightEl) resizeShrinkRightEl.value = 0;
    updateResizePreview();
  });
  if (resizeShrinkLeftEl) resizeShrinkLeftEl.addEventListener("input", () => {
    if (resizeLeftEl) resizeLeftEl.value = 0;
    if (resizeRightEl) resizeRightEl.value = 0;
    const valL = parseFloat(resizeShrinkLeftEl.value || "0");
    const valR = parseFloat(resizeShrinkRightEl.value || "0");
    if (resizeState && valL + valR > resizeState.shrinkMax) {
      resizeShrinkRightEl.value = (resizeState.shrinkMax - valL).toFixed(3);
    }
    updateResizePreview();
  });
  if (resizeShrinkRightEl) resizeShrinkRightEl.addEventListener("input", () => {
    if (resizeLeftEl) resizeLeftEl.value = 0;
    if (resizeRightEl) resizeRightEl.value = 0;
    const valL = parseFloat(resizeShrinkLeftEl.value || "0");
    const valR = parseFloat(resizeShrinkRightEl.value || "0");
    if (resizeState && valL + valR > resizeState.shrinkMax) {
      resizeShrinkLeftEl.value = (resizeState.shrinkMax - valR).toFixed(3);
    }
    updateResizePreview();
  });
  if (resizeApplyEl) resizeApplyEl.addEventListener("click", applyResize);
  if (moveModalEl) {
    moveModalEl.addEventListener("click", (event) => {
      if (event.target === moveModalEl) {
        closeMoveModal();
      }
    });
  }
  if (moveCloseEl) moveCloseEl.addEventListener("click", closeMoveModal);
  if (moveCancelEl) moveCancelEl.addEventListener("click", closeMoveModal);
  if (moveSliderEl) moveSliderEl.addEventListener("input", updateMovePreview);
  if (moveApplyEl) moveApplyEl.addEventListener("click", applyMove);

  let isDraggingPreview = false;
  let dragStartX = 0;
  let dragStartOffset = 0;

  if (moveBlockPartEl) {
    moveBlockPartEl.addEventListener("pointerdown", (e) => {
      if (!moveState) return;
      moveBlockPartEl.setPointerCapture(e.pointerId);
      dragStartX = e.clientX;
      dragStartOffset = parseFloat(moveSliderEl.value || "0");
      isDraggingPreview = true;
      moveBlockPartEl.classList.replace("cursor-grab", "cursor-grabbing");
    });

    moveBlockPartEl.addEventListener("pointermove", (e) => {
      if (!isDraggingPreview || !moveState) return;
      const dx = e.clientX - dragStartX;
      const containerWidth = moveLayoutPreviewEl.clientWidth - 12;
      
      const totalGb = moveState.beforeMax + moveState.baseSize + moveState.afterMax;
      if (containerWidth <= 0 || totalGb <= 0) return;
      
      const gbPerPixel = totalGb / containerWidth;
      let newOffset = dragStartOffset + (dx * gbPerPixel);
      
      newOffset = Math.max(-moveState.beforeMax, Math.min(moveState.afterMax, newOffset));
      
      moveSliderEl.value = newOffset.toFixed(3);
      updateMovePreview();
    });

    const endDrag = (e) => {
      if (!isDraggingPreview) return;
      isDraggingPreview = false;
      moveBlockPartEl.releasePointerCapture(e.pointerId);
      moveBlockPartEl.classList.replace("cursor-grabbing", "cursor-grab");
    };

    moveBlockPartEl.addEventListener("pointerup", endDrag);
    moveBlockPartEl.addEventListener("pointercancel", endDrag);
  }
  if (createCloseEl) createCloseEl.addEventListener("click", closeCreateModal);
  if (createCancelEl) createCancelEl.addEventListener("click", closeCreateModal);
  if (createTargetEl) createTargetEl.addEventListener("change", updateCreatePreview);
  if (createFsEl) createFsEl.addEventListener("change", updateCreatePreview);
  if (createSizeRangeEl) createSizeRangeEl.addEventListener("input", () => syncCreateSizeInputs("range"));
  if (createSizeInputEl) createSizeInputEl.addEventListener("input", () => syncCreateSizeInputs("input"));
  if (createLabelSelectEl) createLabelSelectEl.addEventListener("change", updateCreatePreview);
  if (createQueueEl) createQueueEl.addEventListener("click", queueCreate);
  if (applyCloseEl) applyCloseEl.addEventListener("click", closeApplyModal);
  if (applyDismissEl) applyDismissEl.addEventListener("click", closeApplyModal);
  const applyCopyLogsBtn = document.getElementById("apply-copy-logs");
  if (applyCopyLogsBtn) {
    applyCopyLogsBtn.addEventListener("click", () => {
      const header = [
        '='.repeat(72),
        `  OpenPart — Execution Log`,
        `  Captured : ${new Date().toISOString()}`,
        `  Entries  : ${applyLogs.length}`,
        '='.repeat(72),
        '',
      ].join('\n');

      const body = applyLogs
        .map(e => `[${e.time}] [${e.level.toUpperCase().padEnd(5)}] ${e.message}`)
        .join('\n');

      const footer = [
        '',
        '='.repeat(72),
        '  End of log',
        '='.repeat(72),
      ].join('\n');

      const fullText = header + body + footer;

      navigator.clipboard.writeText(fullText)
        .then(() => {
          applyCopyLogsBtn.textContent = "Copied!";
          setTimeout(() => {
            applyCopyLogsBtn.textContent = "Copy Logs";
          }, 2000);
        })
        .catch(err => {
          console.error("Clipboard copy failed:", err);
          alert("Failed to copy logs: " + err);
        });
    });
  }
  setupApplyTabs();
  if (infoCloseEl) infoCloseEl.addEventListener("click", closeInfoModal);
  if (infoCancelEl) infoCancelEl.addEventListener("click", closeInfoModal);
  if (infoDismissEl) infoDismissEl.addEventListener("click", () => {
    if (infoConfirmHandler) {
      const handler = infoConfirmHandler;
      infoConfirmHandler = null;
      closeInfoModal();
      handler();
      return;
    }
    closeInfoModal();
  });

  if (diskSelectEl) {
    diskSelectEl.addEventListener("change", async (e) => {
      selectedDiskIndex = parseInt(e.target.value, 10);
      selectedId = null;
      state = mapFromCore(coreStateRaw);
      render();
      if (state.queue.length > 0) {
        await updateComposedState();
      }
    });
  }

  if (refreshBtn) refreshBtn.addEventListener("click", () => {
    if (!invoke) return;
    refreshState({ logMessage: "Refreshed disk state.", button: refreshBtn });
  });

  if (restoreBtn) restoreBtn.addEventListener("click", async () => {
    if (!invoke) return alert("Backend not available.");
    try {
      const backups = await invoke("get_backups_command");
      if (!backups || backups.length === 0) {
        openInfoModal("Restore Layout", ["No saved layout snapshots found.", "Snapshots are automatically created before applying partition changes."]);
        return;
      }
      openRestoreModal(backups);
    } catch (e) {
      alert("Failed to fetch backups: " + e);
    }
  });

  if (settingsBtn) settingsBtn.addEventListener("click", () => {
    openInfoModal("Settings", ["Settings are not available yet."]);
  });
  if (helpBtn) helpBtn.addEventListener("click", () => {
    openInfoModal("Help", ["Drag partitions to move them.", "Click a partition to resize it.", "Queue changes and apply when ready."]);
  });
  if (infoBtn) infoBtn.addEventListener("click", () => {
    openInfoModal("About OpenPart", ["Native partition manager UI.", "Built with Rust and Tauri."]);
  });
  if (discardBtn) discardBtn.addEventListener("click", () => {
    if (!confirm("Discard all queued operations?")) return;
    state.queue = [];
    renderQueue();
    saveQueue();
    if (coreStateRaw) {
      state = mapFromCore(coreStateRaw);
      render();
    }
  });

  if (applyBtn) applyBtn.addEventListener("click", async () => {
    if (!invoke) return alert("Backend not available.");
    if (!coreStateRaw) return alert("No disk state available.");
    const stageTotal = state.queue.length + 2;
    openApplyModal(stageTotal);
    if (!state.queue || state.queue.length === 0) {
      setApplyStatus("No queued operations to apply.");
      addApplyLog("info", "Queue is empty.");
      finishApply();
      return;
    }

    let preflightBatteryWarn = false;
    if (navigator.getBattery) {
      try {
        const battery = await navigator.getBattery();
        if (!battery.charging) {
          preflightBatteryWarn = true;
        }
      } catch (e) {
        console.warn("Could not query battery status via Web API:", e);
      }
    }

    let confirmMsg = "This will execute queued changes on the disk. Are you sure?";
    if (preflightBatteryWarn) {
      confirmMsg = [
        "WARNING: You are currently running on battery power.",
        "Moving/modifying partition sectors requires a constant power source. Power loss during execution will cause permanent data loss.",
        "",
        "It is highly recommended to connect AC power before proceeding.",
        "Do you want to proceed on battery anyway?"
      ].join("\n");
    }

    if (!confirm(confirmMsg)) {
      setApplyStatus("Cancelled.");
      addApplyLog("info", "Apply cancelled by user.");
      finishApply();
      return;
    }

    // Pre-emptively approve running on battery if user confirmed the warning
    if (preflightBatteryWarn) {
      state.queue.forEach(item => {
        if (item.inputs) item.inputs.allow_battery = true;
      });
    }

    setButtonLoading(applyBtn, true);
    applyInProgress = true;
    setApplyStatus("Applying changes...");
    setApplyProgress(1, stageTotal);
    addApplyLog("info", "Building execution plan.");

    let completed = 0;
    const queueToProcess = [...state.queue];
    for (let idx = 0; idx < queueToProcess.length; idx += 1) {
      const item = queueToProcess[idx];
      if (!item.inputs) {
        console.warn("Skipping queue item without inputs:", item);
        item.status = "Failed";
        addApplyLog("error", `Missing inputs for ${item.title}.`);
        renderQueue();
        completed += 1;
        setApplyProgress(1 + completed, stageTotal);
        continue;
      }

      let attempt = 0;
      let success = false;
      while (attempt < 2 && !success) {
        try {
          startApplyProgressTicker(1 + completed, stageTotal);
          addApplyLog("info", `Applying: ${item.title}`);
          
          let lastReport = null;
          const inputsArray = Array.isArray(item.inputs) ? item.inputs : [item.inputs];
          for (let i = 0; i < inputsArray.length; i++) {
            const input = inputsArray[i];
            if (inputsArray.length > 1) {
              addApplyLog("info", `Applying sub-task ${i + 1}/${inputsArray.length} for ${item.title}`);
            }
            const report = await invoke("apply_plan_command", { inputs: input, dryRun: false });
            console.log("Apply result:", report);
            if (report && report.ok) {
              if (report.steps && report.steps.length > 0) {
                report.steps.forEach((step) => addApplyLog("info", formatApplyStep(step)));
              }
              if (report.warnings && report.warnings.length > 0) {
                report.warnings.forEach((warning) => addApplyLog("warning", `Warning: ${warning}`));
              }
            } else {
              throw new Error(report && report.steps ? report.steps.join("\n") : "Operation failed");
            }
            lastReport = report;
          }

          stopApplyProgressTicker();
          setApplyProgress(1 + completed, stageTotal);
          item.status = "Applied";
          addLog("success", `Applied: ${item.title}`);
          addApplyLog("success", `Applied: ${item.title}`);
          renderQueue();
          // remove applied item
          state.queue = state.queue.filter((q) => q.id !== item.id);
          renderQueue();
          completed += 1;
          saveQueue();
          success = true;
        } catch (err) {
          const errStr = String(err);
          if (errStr.includes("PREFLIGHT_FAIL: Running on battery") && attempt === 0) {
            stopApplyProgressTicker();
            const proceed = confirm([
              "PRE-FLIGHT WARNING: Running on battery power.",
              "Moving/modifying partition sectors requires a constant power source. Power loss during execution will cause permanent data loss.",
              "",
              "Would you like to bypass this safety check and proceed on battery anyway?"
            ].join("\n"));
            if (proceed) {
              if (Array.isArray(item.inputs)) {
                item.inputs.forEach(input => { input.allow_battery = true; });
              } else {
                item.inputs.allow_battery = true;
              }
              addApplyLog("warning", "User approved running on battery. Bypassing safety check...");
              attempt += 1;
              continue; // retry
            }
          }

          console.error("Apply failed:", err);
          item.status = "Failed";
          addLog("error", `Apply failed: ${item.title}`);
          addApplyLog("error", `Apply failed: ${item.title}`);
          addApplyLog("error", String(err));
          renderQueue();
          setApplyStatus("Apply failed.");
          stopApplyProgressTicker();
          setApplyProgress(stageTotal, stageTotal);
          finishApply();
          return;
        }
      }
    }
    stopApplyProgressTicker();
    setApplyProgress(stageTotal, stageTotal);
    setApplyStatus("Apply complete.");
    addApplyLog("info", "Refreshing disk state...");
    await refreshState({ logMessage: "Disk state refreshed after apply.", silent: true });
    saveQueue();
    finishApply({ complete: true });
  });

  if (actionResizeBtn) actionResizeBtn.addEventListener("click", () => {
    const segment = getSelectedSegment();
    if (!segment) {
      openInfoModal("Move partition", ["Select a partition first.", "You can also click a partition block directly to resize/move it."]);
      return;
    }
    if (segment.kind === "unallocated") {
      openInfoModal("Move partition", ["Select an allocated partition, not unallocated space."]);
      return;
    }
    openMoveModal(segment);
  });

  if (actionNewBtn) actionNewBtn.addEventListener("click", () => {
    openCreateModal();
  });

  if (actionDeleteBtn) actionDeleteBtn.addEventListener("click", () => {
    const segment = getSelectedSegment();
    if (!segment) {
      alert("Select a partition to delete.");
      return;
    }
    if (segment.kind === "unallocated") {
      alert("Select a real partition, not unallocated space.");
      return;
    }
    if (!confirm(`Queue delete for ${labelFor(segment)}?`)) return;

    const inputs = buildInputs({
      action: { Delete: { target: segment.letter || `${segment.index}` } }
    });

    enqueue({
      title: `Delete ${labelFor(segment)}`,
      details: `Delete partition ${labelFor(segment)}.`,
      status: "Pending Apply",
      inputs
    });
    addLog("info", `Queued delete: ${labelFor(segment)}.`);
    updateComposedState();
  });

  if (actionFormatBtn) actionFormatBtn.addEventListener("click", () => {
    const segment = getSelectedSegment();
    if (!segment) {
      alert("Select a partition to format.");
      return;
    }
    if (!segment.letter) {
      alert("Formatting requires a drive letter.");
      return;
    }
    if (!confirm(`Format ${labelFor(segment)}? This will erase data on the partition.`)) return;
    const fs = prompt("File system (NTFS/exFAT)", segment.fs || "NTFS");
    if (!fs) return;
    const label = prompt("Volume label (optional)", segment.label || "");

    const inputs = buildInputs({
      action: { Format: { target: segment.letter, fs, label: label || null } }
    });

    enqueue({
      title: `Format ${labelFor(segment)}`,
      details: `Format as ${fs}.`,
      status: "Pending Apply",
      inputs
    });
    renderQueue();
  });

  if (actionPropsBtn) actionPropsBtn.addEventListener("click", () => {
    const segment = getSelectedSegment();
    if (!segment) {
      openInfoModal("Properties", ["Select a partition to view properties."]);
      return;
    }
    openPropertiesModal(segment);
  });

  function mapFromCore(coreState) {
    let diskIdx = selectedDiskIndex;
    if (coreState && Array.isArray(coreState.disks)) {
      if (diskIdx >= coreState.disks.length) {
        diskIdx = 0;
        selectedDiskIndex = 0;
      }
    }
    const disk = coreState && Array.isArray(coreState.disks) ? coreState.disks[diskIdx] : null;
    if (!disk) {
      return {
        totalSizeGb: 0,
        diskLabel: "No disks detected",
        diskModel: "PowerShell disk scan returned no devices.",
        diskStyle: "MBR",
        diskStatus: "Offline",
        diskHealth: "Unknown",
        segments: [],
        queue: Array.isArray(coreState && coreState.queue) ? coreState.queue.map((item) => ({
          id: item.id,
          title: item.op_type,
          details: item.details,
          status: "Pending Apply"
        })) : []
      };
    }
    const segments = disk.partitions
      .filter((part) => Number(part.size_gb) > 0)
      .filter((part) => part.kind !== "Unallocated" || part.size_gb >= MIN_UNALLOCATED_GB)
      .map((part) => {
      const letter = part.letter || null;
      const label = part.label || "Partition";
      const kind = part.kind.toLowerCase();
      return {
        id: `${label}-${letter || part.index}`,
        index: part.index,
        label,
        letter,
        fs: part.file_system,
        sizeGb: part.size_gb,
        usedGb: part.used_gb,
        kind,
        tone: toneForKind(kind, letter),
        status: part.status
      };
    });

    return {
      totalSizeGb: disk.size_gb,
      diskLabel: disk.label,
      diskModel: disk.model,
      diskStyle: disk.style === "Gpt" ? "GPT" : "MBR",
      diskStatus: disk.status || "Online",
      diskHealth: "Healthy",
      segments,
      queue: Array.isArray(coreState.queue) ? coreState.queue.map((item) => ({
        id: item.id,
        title: item.op_type,
        details: item.details,
        status: "Pending Apply"
      })) : []
    };
  }

  function toneForKind(kind, letter) {
    if (kind === "system") return "tertiary";
    if (kind === "unallocated") return "surface";
    if (letter === "D") return "secondary";
    if (letter === "E") return "primary-container";
    return "primary";
  }

  function render() {
    renderHeader();
    renderDiskMap();
    renderTable();
    renderQueue();
  }

  function renderHeader() {
    if (diskSelectEl && coreStateRaw && Array.isArray(coreStateRaw.disks)) {
      const currentOptions = Array.from(diskSelectEl.options).map(o => o.value);
      const newOptions = coreStateRaw.disks.map((d, i) => String(i));
      if (currentOptions.join(",") !== newOptions.join(",")) {
        diskSelectEl.innerHTML = "";
        coreStateRaw.disks.forEach((disk, idx) => {
          const opt = document.createElement("option");
          opt.value = String(idx);
          opt.textContent = disk.label || `Disk ${idx}`;
          opt.className = "bg-surface-container-highest text-on-surface";
          diskSelectEl.appendChild(opt);
        });
      }
      diskSelectEl.value = String(selectedDiskIndex);
    }
    if (diskStyleBadgeEl) {
      diskStyleBadgeEl.textContent = state.diskStyle;
    }
    if (diskSubtitleEl) {
      diskSubtitleEl.textContent = state.diskModel;
    }
    if (diskStatusEl) {
      diskStatusEl.textContent = state.diskStatus;
    }
    if (diskHealthEl) {
      diskHealthEl.textContent = state.diskHealth;
    }
  }

  function renderDiskMap() {
    mapEl.innerHTML = "";
    state.segments.forEach((segment, index) => {
      const block = document.createElement("div");
      block.className = buildSegmentClass(segment, index);
      block.dataset.id = segment.id;
      block.style.flex = `${segment.sizeGb} 1 0%`;
      block.style.minWidth = segment.kind === "system" ? "52px" : "70px";

      const label = document.createElement("div");
      label.className = "partition-label";
      const name = segment.letter ? `${segment.label} (${segment.letter}:)` : segment.label;
      label.innerHTML = `<div class="label-title">${name}</div><div class="label-sub">${formatSize(
        segment.sizeGb
      )} ${segment.fs}</div>`;

      if (segment.kind === "unallocated") {
        label.innerHTML = `<div class="label-title">Unallocated</div><div class="label-sub">${formatSize(
          segment.sizeGb
        )}</div>`;
      }

      block.appendChild(label);
      mapEl.appendChild(block);
    });
  }

  function buildSegmentClass(segment, index) {
    const classes = [
      "partition-block",
      `tone-${segment.tone}`,
      segment.kind === "unallocated" ? "is-unallocated stripes-bg" : "",
      index === 0 ? "rounded-left" : "",
      index === state.segments.length - 1 ? "rounded-right" : "",
      selectedId === segment.id ? "is-selected" : "",
      dragState && dragState.id === segment.id && dragState.moved ? "is-dragging" : ""
    ];
    return classes.filter(Boolean).join(" ");
  }

  function renderTable() {
    tableEl.innerHTML = "";
    state.segments.forEach((segment) => {
      const row = document.createElement("tr");
      row.className = `table-row${segment.id === selectedId ? " is-selected" : ""}`;
      row.dataset.id = segment.id;
      row.addEventListener("click", () => {
        if (segment.kind === "unallocated") return;
        selectedId = segment.id;
        renderDiskMap();
        renderTable();
      });

      const labelCell = document.createElement("td");
      labelCell.className = "p-table-cell-padding h-12 flex items-center gap-3";
      const icon = document.createElement("span");
      icon.className = `material-symbols-outlined text-[20px] ${iconColor(segment)}`;
      icon.textContent = iconForSegment(segment);
      const label = document.createElement("span");
      label.textContent = segment.letter
        ? `${segment.label} (${segment.letter}:)`
        : segment.label;
      labelCell.appendChild(icon);
      labelCell.appendChild(label);

      const fsCell = document.createElement("td");
      fsCell.className =
        "p-table-cell-padding h-12 font-label-mono text-label-mono text-on-surface-variant";
      fsCell.textContent = segment.fs;

      const sizeCell = document.createElement("td");
      sizeCell.className =
        "p-table-cell-padding h-12 font-label-mono text-label-mono text-right text-on-surface-variant";
      sizeCell.textContent = formatSize(segment.sizeGb);

      const usedCell = document.createElement("td");
      usedCell.className = "p-table-cell-padding h-12";
      usedCell.appendChild(buildUsage(segment));

      const statusCell = document.createElement("td");
      statusCell.className = "p-table-cell-padding h-12";
      statusCell.appendChild(buildStatus(segment));

      row.appendChild(labelCell);
      row.appendChild(fsCell);
      row.appendChild(sizeCell);
      row.appendChild(usedCell);
      row.appendChild(statusCell);
      tableEl.appendChild(row);
    });
  }

  function buildUsage(segment) {
    if (segment.kind === "unallocated") {
      const empty = document.createElement("div");
      empty.className = "text-on-surface-variant font-label-mono";
      empty.textContent = "-";
      return empty;
    }

    const wrap = document.createElement("div");
    wrap.className = "flex flex-col justify-center w-full max-w-[220px] gap-1.5";
    const bar = document.createElement("div");
    bar.className = "w-full h-1.5 bg-surface-variant rounded-full overflow-hidden";
    const fill = document.createElement("div");
    const usedPct = segment.sizeGb > 0 ? (segment.usedGb / segment.sizeGb) * 100 : 0;
    const clamped = Math.max(0, Math.min(100, usedPct));
    fill.className = `h-full ${fillColor(segment)}`;
    fill.style.width = `${clamped.toFixed(0)}%`;
    bar.appendChild(fill);

    const meta = document.createElement("div");
    meta.className =
      "flex justify-between font-label-mono text-[11px] text-on-surface-variant";
    const free = Math.max(0, segment.sizeGb - segment.usedGb);
    meta.innerHTML = `<span>${clamped.toFixed(0)}%</span><span>${formatSize(free)} free</span>`;

    wrap.appendChild(bar);
    wrap.appendChild(meta);
    return wrap;
  }

  function buildStatus(segment) {
    const badge = document.createElement("span");
    if (segment.kind === "unallocated") {
      badge.className = "text-outline-variant font-label-mono";
      badge.textContent = "-";
      return badge;
    }
    const statusLower = segment.status.toLowerCase();
    const styleClass = statusLower.includes("pending")
      ? "bg-primary-container text-on-primary-container"
      : "bg-secondary-container text-on-secondary-container";

    badge.className = `inline-flex items-center px-2 py-1 rounded-md text-[11px] font-bold ${styleClass}`;
    badge.textContent = segment.status;
    return badge;
  }

  function iconForSegment(segment) {
    if (segment.kind === "system") return "dvr";
    if (segment.kind === "unallocated") return "help_outline";
    return segment.letter === "D" ? "folder_special" : "desktop_windows";
  }

  function iconColor(segment) {
    if (segment.kind === "system") return "text-tertiary";
    if (segment.letter === "D") return "text-secondary";
    if (segment.kind === "unallocated") return "text-on-surface-variant";
    return "text-primary";
  }

  function fillColor(segment) {
    if (segment.kind === "system") return "bg-tertiary";
    if (segment.letter === "D") return "bg-secondary";
    return "bg-primary";
  }

  function renderQueue() {
    queueEl.innerHTML = "";
    queueCountEl.textContent = state.queue.length.toString();
    if (state.queue.length === 0) {
      queueEmptyEl.classList.remove("hidden");
      return;
    }

    queueEmptyEl.classList.add("hidden");
    state.queue.forEach((item) => {
      const card = document.createElement("div");
      card.className =
        "queue-card bg-surface border border-outline-variant rounded-lg p-4 shadow-sm relative overflow-hidden";
      card.innerHTML = `
        <div class="absolute left-0 top-0 bottom-0 w-1.5 bg-primary"></div>
        <div class="flex items-start gap-3">
          <div class="mt-0.5 text-primary">
            <span class="material-symbols-outlined text-[20px]">format_h3</span>
          </div>
          <div class="flex-1">
            <h4 class="font-label-md font-semibold text-on-surface leading-tight mb-1.5">${item.title}</h4>
            <p class="font-label-mono text-[11px] text-on-surface-variant mb-3 leading-relaxed">${item.details}</p>
            <div class="flex justify-between items-center text-[11px] font-label-mono">
              <span class="text-primary font-bold bg-primary-fixed text-on-primary-fixed px-2 py-1 rounded-md">${item.status}</span>
              <button class="text-error hover:text-on-error-container hover:bg-error-container px-2 py-1 rounded-md transition-colors flex items-center gap-1" data-action="cancel" data-id="${item.id}">
                <span class="material-symbols-outlined text-[14px]">close</span>
                Cancel
              </button>
            </div>
          </div>
        </div>
      `;
      queueEl.appendChild(card);
    });
  }


  function openApplyModal(total) {
    if (!applyModalEl) return;
    applyLogs = [];
    renderApplyLogs();
    applyProgressCurrent = 0;
    applyProgressTotal = total || 0;
    if (applyProgressFillEl) applyProgressFillEl.classList.remove("is-animating");
    setApplyProgress(0, total || 0);
    setApplyStatus("Preparing...");
    applyModalEl.classList.remove("hidden");
    applyInProgress = false;

    const copyLogsBtn = document.getElementById("apply-copy-logs");
    if (copyLogsBtn) {
      copyLogsBtn.disabled = true;
      copyLogsBtn.textContent = "Copy Logs";
    }
  }

  function closeApplyModal() {
    if (!applyModalEl || applyInProgress) return;
    applyModalEl.classList.add("hidden");
  }

  function setApplyProgress(completed, total) {
    applyProgressCurrent = completed;
    applyProgressTotal = total;
    const pct = total > 0 ? (completed / total) * 100 : 0;
    if (applyProgressFillEl) applyProgressFillEl.style.width = `${pct}%`;
    if (applyPercentEl) applyPercentEl.textContent = `${Math.round(pct)}%`;
    if (applyCountEl) applyCountEl.textContent = `${Math.floor(completed)}/${total}`;
  }

  function startApplyProgressTicker(stageBase, total) {
    stopApplyProgressTicker();
    applyProgressTotal = total;
    applyProgressCurrent = Math.max(applyProgressCurrent, stageBase);
    if (applyProgressFillEl) applyProgressFillEl.classList.add("is-animating");
    applyProgressTimer = setInterval(() => {
      const target = Math.min(total, stageBase + 0.9);
      if (applyProgressCurrent >= target) return;
      applyProgressCurrent = Math.min(target, applyProgressCurrent + 0.02);
      setApplyProgress(applyProgressCurrent, total);
    }, 120);
  }

  function stopApplyProgressTicker() {
    if (applyProgressTimer) clearInterval(applyProgressTimer);
    applyProgressTimer = null;
    if (applyProgressFillEl) applyProgressFillEl.classList.remove("is-animating");
  }

  function setApplyStatus(text) {
    if (applyStatusEl) applyStatusEl.textContent = text;
  }

  function updateApplyLogCounts() {
    const opsCount = applyLogs.filter(e => e.level !== 'error' && e.level !== 'warning').length;
    const warnCount = applyLogs.filter(e => e.level === 'warning').length;
    const errCount = applyLogs.filter(e => e.level === 'error').length;

    const badgeOps = document.getElementById("badge-apply-ops");
    const badgeWarn = document.getElementById("badge-apply-warnings");
    const badgeErr = document.getElementById("badge-apply-errors");

    if (badgeOps) {
      badgeOps.textContent = opsCount;
      badgeOps.classList.toggle("hidden", opsCount === 0);
    }
    if (badgeWarn) {
      badgeWarn.textContent = warnCount;
      badgeWarn.classList.toggle("hidden", warnCount === 0);
    }
    if (badgeErr) {
      badgeErr.textContent = errCount;
      badgeErr.classList.toggle("hidden", errCount === 0);
    }

    const emptyOps = document.getElementById("apply-log-ops-empty");
    const emptyWarn = document.getElementById("apply-log-warnings-empty");
    const emptyErr = document.getElementById("apply-log-errors-empty");

    if (emptyOps) emptyOps.classList.toggle("hidden", opsCount > 0);
    if (emptyWarn) emptyWarn.classList.toggle("hidden", warnCount > 0);
    if (emptyErr) emptyErr.classList.toggle("hidden", errCount > 0);
  }

  function addApplyLog(level, message) {
    const time = new Date().toLocaleTimeString();
    const entry = { level, message, time };
    applyLogs.push(entry);
    if (applyLogs.length > 200) {
      const removed = applyLogs.shift();
      const targetPanel = removed.level === "error" ? applyLogErrorsEl : (removed.level === "warning" ? applyLogWarningsEl : applyLogOpsEl);
      if (targetPanel && targetPanel.children.length > 1) {
        const firstRow = targetPanel.querySelector("div:not([id])");
        if (firstRow) targetPanel.removeChild(firstRow);
      }
    }
    appendApplyLogToDOM(entry);
  }

  function appendApplyLogToDOM(entry) {
    if (!applyLogOpsEl || !applyLogWarningsEl || !applyLogErrorsEl) return;
    const row = document.createElement("div");
    const levelClass = entry.level === "error"
      ? "text-error"
      : entry.level === "success"
        ? "text-primary"
        : entry.level === "warning"
          ? "text-on-tertiary-fixed-variant"
          : "text-on-surface-variant";
    row.className = levelClass;
    row.textContent = `[${entry.time}] ${entry.message}`;

    if (entry.level === "error") {
      applyLogErrorsEl.appendChild(row);
      applyLogErrorsEl.scrollTop = applyLogErrorsEl.scrollHeight;
      const tabErr = document.getElementById("tab-apply-errors");
      if (tabErr) tabErr.click();
    } else if (entry.level === "warning") {
      applyLogWarningsEl.appendChild(row);
      applyLogWarningsEl.scrollTop = applyLogWarningsEl.scrollHeight;
      const panelErr = document.getElementById("panel-apply-errors");
      if (panelErr && panelErr.classList.contains("hidden")) {
        const tabWarn = document.getElementById("tab-apply-warnings");
        if (tabWarn) tabWarn.click();
      }
    } else {
      applyLogOpsEl.appendChild(row);
      applyLogOpsEl.scrollTop = applyLogOpsEl.scrollHeight;
    }
    updateApplyLogCounts();
  }

  function renderApplyLogs() {
    if (!applyLogOpsEl || !applyLogWarningsEl || !applyLogErrorsEl) return;
    
    applyLogOpsEl.innerHTML = '<div id="apply-log-ops-empty" class="text-[12px] text-outline-variant">No operation output yet.</div>';
    applyLogWarningsEl.innerHTML = '<div id="apply-log-warnings-empty" class="text-[12px] text-outline-variant">No warnings.</div>';
    applyLogErrorsEl.innerHTML = '<div id="apply-log-errors-empty" class="text-[12px] text-outline-variant">No errors.</div>';

    applyLogs.forEach((entry) => {
      const row = document.createElement("div");
      const levelClass = entry.level === "error"
        ? "text-error"
        : entry.level === "success"
          ? "text-primary"
          : entry.level === "warning"
            ? "text-on-tertiary-fixed-variant"
            : "text-on-surface-variant";
      row.className = levelClass;
      row.textContent = `[${entry.time}] ${entry.message}`;

      if (entry.level === "error") {
        applyLogErrorsEl.appendChild(row);
      } else if (entry.level === "warning") {
        applyLogWarningsEl.appendChild(row);
      } else {
        applyLogOpsEl.appendChild(row);
      }
    });

    updateApplyLogCounts();
    
    const tabOps = document.getElementById("tab-apply-ops");
    if (tabOps) tabOps.click();
  }

  function setupApplyTabs() {
    const tabOps = document.getElementById("tab-apply-ops");
    const tabWarn = document.getElementById("tab-apply-warnings");
    const tabErr = document.getElementById("tab-apply-errors");

    const panelOps = document.getElementById("panel-apply-ops");
    const panelWarn = document.getElementById("panel-apply-warnings");
    const panelErr = document.getElementById("panel-apply-errors");

    if (!tabOps || !tabWarn || !tabErr || !panelOps || !panelWarn || !panelErr) return;

    const tabs = [tabOps, tabWarn, tabErr];
    const panels = [panelOps, panelWarn, panelErr];

    function setActiveTab(index) {
      tabs.forEach((tab, i) => {
        if (i === index) {
          tab.className = "font-label-md font-semibold text-primary border-b-2 border-primary pb-2 flex items-center gap-1.5 active:scale-95 transition-all focus:outline-none";
          panels[i].classList.remove("hidden");
        } else {
          tab.className = "font-label-md font-semibold text-outline hover:text-on-surface pb-2 flex items-center gap-1.5 active:scale-95 transition-all focus:outline-none";
          panels[i].classList.add("hidden");
        }
      });
    }

    tabOps.addEventListener("click", () => setActiveTab(0));
    tabWarn.addEventListener("click", () => setActiveTab(1));
    tabErr.addEventListener("click", () => setActiveTab(2));
  }

  function formatApplyStep(step) {
    const text = String(step || "").trim();
    const planPrefixes = [
      "Load disk",
      "Chunk size",
      "Resize ",
      "Move ",
      "Create ",
      "Delete ",
      "Convert ",
      "Assign ",
      "Format ",
      "Clone ",
      "Wipe ",
      "Mark ",
      "Use test VHD",
      "Restore GPT",
      "Move unallocated",
      "Target LBA",
      "Target sectors",
      "Format filesystem"
    ];
    if (planPrefixes.some((prefix) => text.startsWith(prefix))) {
      return `Plan: ${text}`;
    }
    if (text.startsWith("Applied script:")) {
      return `Executor: ${text}`;
    }
    if (text.startsWith("Result size bytes:")) {
      return `Executor: ${text}`;
    }
    return text;
  }

  function finishApply({ complete } = {}) {
    applyInProgress = false;
    stopApplyProgressTicker();
    if (applyProgressFillEl) applyProgressFillEl.classList.remove("is-animating");
    if (complete && applyProgressTotal > 0) {
      setApplyProgress(applyProgressTotal, applyProgressTotal);
    }
    setButtonLoading(applyBtn, false);

    const copyLogsBtn = document.getElementById("apply-copy-logs");
    if (copyLogsBtn) {
      copyLogsBtn.disabled = false;
    }
  }

  function openInfoModal(title, lines, subtitle, options = {}) {
    if (!infoModalEl || !infoTitleEl || !infoBodyEl) return;
    infoTitleEl.textContent = title || "Info";
    if (infoSubtitleEl) infoSubtitleEl.textContent = subtitle || "Details";
    infoBodyEl.innerHTML = "";
    (lines || []).forEach((line) => {
      const row = document.createElement("div");
      row.className = "info-line";
      const bullet = document.createElement("span");
      bullet.className = "bullet";
      const text = document.createElement("span");
      text.textContent = line;
      row.appendChild(bullet);
      row.appendChild(text);
      infoBodyEl.appendChild(row);
    });
    infoConfirmHandler = options.onConfirm || null;
    if (infoDismissEl) infoDismissEl.textContent = options.confirmText || "OK";
    if (infoCancelEl) {
      infoCancelEl.textContent = options.cancelText || "Cancel";
      infoCancelEl.classList.toggle("hidden", !infoConfirmHandler);
    }
    infoModalEl.classList.remove("hidden");
  }

  function closeInfoModal() {
    if (!infoModalEl) return;
    infoModalEl.classList.add("hidden");
    infoConfirmHandler = null;
    if (infoDismissEl) infoDismissEl.textContent = "OK";
    if (infoCancelEl) infoCancelEl.classList.add("hidden");
  }

  function openRestoreModal(backups) {
    if (!infoModalEl || !infoTitleEl || !infoBodyEl) return;
    infoTitleEl.textContent = "Restore Disk Layout";
    if (infoSubtitleEl) infoSubtitleEl.textContent = "Select a saved snapshot to recover partition table boundaries:";
    infoBodyEl.innerHTML = "";

    const container = document.createElement("div");
    container.className = "flex flex-col gap-3 w-full";

    const label = document.createElement("label");
    label.className = "text-[12px] font-semibold text-on-surface-variant";
    label.textContent = "Available Snapshots (recent 2)";
    container.appendChild(label);

    const select = document.createElement("select");
    select.className = "resize-range w-full p-2 border border-outline-variant rounded-md bg-surface text-on-surface focus:outline-none";
    
    backups.forEach((b, index) => {
      const option = document.createElement("option");
      option.value = b.timestamp;
      const dateStr = new Date(b.timestamp).toLocaleString();
      option.textContent = `${index + 1}. ${dateStr} - ${b.description}`;
      select.appendChild(option);
    });
    container.appendChild(select);

    const disclaimer = document.createElement("div");
    disclaimer.className = "text-[11px] text-error bg-error-container p-3 rounded-lg border border-error/20 leading-relaxed mt-2";
    disclaimer.innerHTML = "<strong>WARNING:</strong> Restoring partition table boundaries will overwrite current partition indexes and offsets to the saved snapshot states. This is a low-level operation to recover from interrupted moves.";
    container.appendChild(disclaimer);

    infoBodyEl.appendChild(container);

    infoConfirmHandler = async () => {
      const selectedTimestamp = select.value;
      if (!selectedTimestamp) return;

      if (!confirm("Are you sure you want to restore the partition table layout to this snapshot?")) return;

      closeInfoModal();
      openApplyModal(3);
      setApplyStatus("Restoring layout...");
      setApplyProgress(1, 3);
      addApplyLog("info", "Starting partition layout restoration.");

      try {
        startApplyProgressTicker(1, 3);
        const report = await invoke("restore_backup_command", { timestamp: selectedTimestamp });
        stopApplyProgressTicker();
        setApplyProgress(3, 3);
        setApplyStatus("Restore complete.");
        if (report && report.ok) {
          addApplyLog("success", "Restore successfully completed.");
          if (report.steps) {
            report.steps.forEach(s => addApplyLog("info", s));
          }
        } else {
          addApplyLog("error", "Restore failed.");
        }
        await refreshState({ logMessage: "Disk state refreshed after restore.", silent: true });
        finishApply({ complete: true });
      } catch (err) {
        stopApplyProgressTicker();
        setApplyProgress(3, 3);
        setApplyStatus("Restore failed.");
        addApplyLog("error", "Restore failed: " + err);
        finishApply();
      }
    };

    if (infoDismissEl) infoDismissEl.textContent = "Restore";
    if (infoCancelEl) {
      infoCancelEl.textContent = "Cancel";
      infoCancelEl.classList.remove("hidden");
    }
    infoModalEl.classList.remove("hidden");
  }


  function openPropertiesModal(segment) {
    if (!segment) return;
    openInfoModal("Properties", [
      `Name: ${labelFor(segment)}`,
      `File system: ${segment.fs || "-"}`,
      `Size: ${formatSize(segment.sizeGb)}`,
      `Used: ${formatSize(segment.usedGb)}`,
      `Status: ${segment.status}`
    ]);
  }

  function openCreateModal(preSelectedId = null) {
    if (!createModalEl) return;
    
    // If opened via button without selection, try to use selectedId if it's unallocated
    if (!preSelectedId && selectedId) {
      const seg = state.segments.find(s => s.id === selectedId);
      if (seg && seg.kind === "unallocated") {
        preSelectedId = selectedId;
      }
    }

    createState = buildCreateSpaces();
    populateCreateModal(createState, preSelectedId);
    if (createSizeInputEl) createSizeInputEl.value = "";
    updateCreatePreview();
    createModalEl.classList.remove("hidden");
  }

  function closeCreateModal() {
    if (!createModalEl) return;
    createModalEl.classList.add("hidden");
    createState = null;
  }

  function buildCreateSpaces() {
    return state.segments.filter((segment) => segment.kind === "unallocated" && segment.sizeGb >= MIN_UNALLOCATED_GB);
  }

  function populateCreateModal(spaces, preSelectedId) {
    if (!createTargetEl) return;
    createTargetEl.innerHTML = "";
    if (!spaces || spaces.length === 0) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = "No unallocated space available";
      createTargetEl.appendChild(option);
      createTargetEl.disabled = true;
      if (createQueueEl) createQueueEl.disabled = true;
      if (createSpaceCountEl) createSpaceCountEl.textContent = "0";
      if (createSpaceSizeEl) createSpaceSizeEl.textContent = "-";
      if (createSizeRangeEl) createSizeRangeEl.disabled = true;
      if (createSizeInputEl) createSizeInputEl.disabled = true;
      if (createLabelSelectEl) createLabelSelectEl.disabled = true;
      if (createLabelCustomEl) createLabelCustomEl.disabled = true;
      return;
    }

    createTargetEl.disabled = false;
    if (createQueueEl) createQueueEl.disabled = false;

    let preSelectedIndex = 0;
    spaces.forEach((segment, index) => {
      const option = document.createElement("option");
      option.value = String(index);
      option.textContent = `${formatSize(segment.sizeGb)} free space ${index + 1}`;
      createTargetEl.appendChild(option);
      if (preSelectedId === segment.id) {
        preSelectedIndex = index;
      }
    });
    
    createTargetEl.value = String(preSelectedIndex);

    if (createSpaceCountEl) createSpaceCountEl.textContent = String(spaces.length);
    populateCreateLabels();
    if (createSizeRangeEl) createSizeRangeEl.disabled = false;
    if (createSizeInputEl) createSizeInputEl.disabled = false;
    if (createLabelSelectEl) createLabelSelectEl.disabled = false;
    if (createLabelCustomEl) createLabelCustomEl.disabled = false;
  }

  function getSelectedCreateSpace() {
    if (!createState || !createTargetEl) return null;
    const index = Number(createTargetEl.value || "0");
    return createState[index] || createState[0] || null;
  }

  function updateCreatePreview() {
    const space = getSelectedCreateSpace();
    if (createSpaceSizeEl) createSpaceSizeEl.textContent = space ? formatSize(space.sizeGb) : "-";
    if (createSpaceCountEl && createState) createSpaceCountEl.textContent = String(createState.length);
    if (createQueueEl) createQueueEl.disabled = !space;
    if (space && createSizeRangeEl && createSizeInputEl) {
      const max = roundGb(space.sizeGb);
      const min = MIN_UNALLOCATED_GB;
      createSizeRangeEl.min = min.toFixed(3);
      createSizeRangeEl.max = max.toFixed(3);
      createSizeInputEl.min = min.toFixed(3);
      createSizeInputEl.max = max.toFixed(3);
      const current = Number(createSizeInputEl.value || 0);
      const next = current > 0 && current <= max ? current : max;
      createSizeRangeEl.value = next.toFixed(3);
      createSizeInputEl.value = next.toFixed(3);
    }
    if (createLabelSelectEl && createLabelCustomEl) {
      const isCustom = createLabelSelectEl.value === "__custom";
      createLabelCustomEl.disabled = !isCustom;
      createLabelCustomEl.classList.toggle("hidden", !isCustom);
    }
  }

  function populateCreateLabels() {
    if (!createLabelSelectEl) return;
    createLabelSelectEl.innerHTML = "";
    const auto = document.createElement("option");
    auto.value = "";
    auto.textContent = "Auto (assign next)";
    createLabelSelectEl.appendChild(auto);
    const letters = [
      "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
      "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W",
      "X", "Y", "Z"
    ];
    const used = new Set(
      state.segments
        .map((segment) => (segment.letter ? segment.letter.toUpperCase() : null))
        .filter(Boolean)
    );
    const available = letters.filter((letter) => !used.has(letter)).slice(0, 4);
    available.forEach((letter) => {
      const option = document.createElement("option");
      option.value = letter;
      option.textContent = letter;
      createLabelSelectEl.appendChild(option);
    });
    const custom = document.createElement("option");
    custom.value = "__custom";
    custom.textContent = "Custom...";
    createLabelSelectEl.appendChild(custom);
  }

  function syncCreateSizeInputs(source) {
    if (!createSizeRangeEl || !createSizeInputEl) return;
    const space = getSelectedCreateSpace();
    if (!space) return;
    const max = roundGb(space.sizeGb);
    const min = MIN_UNALLOCATED_GB;
    let value = source === "range"
      ? Number(createSizeRangeEl.value || 0)
      : Number(createSizeInputEl.value || 0);
    if (!Number.isFinite(value)) value = max;
    value = Math.max(min, Math.min(max, value));
    createSizeRangeEl.value = value.toFixed(3);
    createSizeInputEl.value = value.toFixed(3);
    updateCreatePreview();
  }

  function queueCreate() {
    const space = getSelectedCreateSpace();
    if (!space) {
      alert("No unallocated space available.");
      return;
    }

    const fs = createFsEl ? createFsEl.value : "NTFS";
    const rawSize = createSizeInputEl ? Number(createSizeInputEl.value || 0) : 0;
    let sizeGb = Number.isFinite(rawSize) && rawSize > 0 ? rawSize : space.sizeGb;

    // Adjust sizeGb if it is close to the total available space or slightly exceeds it due to UI rounding.
    if (sizeGb > space.sizeGb) {
      if (sizeGb <= space.sizeGb + 0.0009) {
        sizeGb = space.sizeGb;
      }
    } else if (sizeGb >= space.sizeGb - MIN_UNALLOCATED_GB) {
      sizeGb = space.sizeGb;
    }

    if (roundGb(sizeGb) > roundGb(space.sizeGb)) {
      alert("Requested size exceeds available unallocated space.");
      return;
    }

    let driveLetter = "";
    if (createLabelSelectEl && createLabelSelectEl.value === "__custom") {
      driveLetter = createLabelCustomEl ? createLabelCustomEl.value.trim().toUpperCase() : "";
    } else if (createLabelSelectEl) {
      driveLetter = createLabelSelectEl.value.trim().toUpperCase();
    }
    if (driveLetter) {
      if (!/^[A-Z]$/.test(driveLetter)) {
        alert("Drive letter must be a single A-Z character.");
        return;
      }
      const used = new Set(
        state.segments
          .map((segment) => (segment.letter ? segment.letter.toUpperCase() : null))
          .filter(Boolean)
      );
      if (used.has(driveLetter)) {
        alert("Selected drive letter is already in use.");
        return;
      }
    }

    const useMax = sizeGb >= space.sizeGb - MIN_UNALLOCATED_GB;
    const planSize = useMax ? 0 : roundGb(sizeGb);
    const inputs = buildInputs({
      action: { Create: { fs, size_gb: planSize, label: null, drive_letter: driveLetter || null } },
      source_unalloc_lba: space.index
    });

    enqueue({
      title: "Create partition",
      details: `Create ${formatSize(sizeGb)} ${fs}.`,
      status: "Pending Apply",
      inputs
    });
    addLog("info", `Queued create: ${fs}.`);
    updateComposedState();
    closeCreateModal();
  }

  function onQueueClick(event) {
    const button = event.target.closest('[data-action="cancel"]');
    if (!button) return;
    const id = Number(button.dataset.id);
    state.queue = state.queue.filter((item) => item.id !== id);
    renderQueue();
    saveQueue();
    updateComposedState();
  }

  function onPointerDown(event) {
    const block = event.target.closest(".partition-block");
    if (!block) return;

    const id = block.dataset.id;
    const segment = state.segments.find((item) => item.id === id);
    if (segment) {
      selectedId = id;
    } else {
      selectedId = null;
    }

    // For unallocated, select it. Do not open create modal automatically.
    if (segment && segment.kind === "unallocated") {
      renderDiskMap();
      renderTable();
      return;
    }

    const rect = mapEl.getBoundingClientRect();
    const children = Array.from(mapEl.children);
    const bounds = children.map((child) => {
      const childRect = child.getBoundingClientRect();
      return {
        left: childRect.left - rect.left,
        right: childRect.right - rect.left,
        width: childRect.width,
        center: (childRect.left + childRect.right) / 2 - rect.left
      };
    });

    dragState = {
      id,
      startX: event.clientX,
      startY: event.clientY,
      startIndex: findIndex(id),
      moved: false,
      originalOrder: state.segments.map((segment) => segment.id),
      bounds
    };

    if (mapEl.setPointerCapture) {
      mapEl.setPointerCapture(event.pointerId);
    }

    renderDiskMap();
    renderTable();
  }

  function onPointerMove(event) {
    if (!dragState) return;

    const dx = event.clientX - dragState.startX;
    const dy = event.clientY - dragState.startY;
    const distance = Math.max(Math.abs(dx), Math.abs(dy));
    if (!dragState.moved && distance < 6) return;

    dragState.moved = true;
    document.body.classList.add("is-dragging");

    const rect = mapEl.getBoundingClientRect();
    const x = event.clientX - rect.left;
    
    // Stable target index lookup using pre-calculated bounding centers
    let targetIndex = -1;
    for (let i = 0; i < dragState.bounds.length; i++) {
      const b = dragState.bounds[i];
      if (x < b.center) {
        targetIndex = i;
        break;
      }
    }
    if (targetIndex === -1) {
      targetIndex = dragState.bounds.length - 1;
    }

    const index = findIndex(dragState.id);
    if (targetIndex !== index && targetIndex >= 0) {
      const [segment] = state.segments.splice(index, 1);
      state.segments.splice(targetIndex, 0, segment);
      
      const children = Array.from(mapEl.children);
      const draggedNode = children[index];
      if (draggedNode) {
        draggedNode.classList.add("is-dragging");
        if (targetIndex < index) {
          mapEl.insertBefore(draggedNode, children[targetIndex]);
        } else {
          mapEl.insertBefore(draggedNode, children[targetIndex].nextSibling);
        }
      }
      
      const newChildren = Array.from(mapEl.children);
      newChildren.forEach((child, i) => {
        child.classList.toggle("rounded-left", i === 0);
        child.classList.toggle("rounded-right", i === newChildren.length - 1);
      });
    }
  }

  async function onPointerUp(event) {
    if (!dragState) return;

    if (mapEl.releasePointerCapture && event) {
      try {
        mapEl.releasePointerCapture(event.pointerId);
      } catch (e) {
        console.warn("Failed to release pointer capture", e);
      }
    }

    const endIndex = findIndex(dragState.id);
    const currentOrder = state.segments.map((segment) => segment.id);
    const orderChanged = dragState.moved && !arraysEqual(currentOrder, dragState.originalOrder);

    if (orderChanged) {
      const draggedSegment = state.segments[endIndex];
      
      // Calculate delta starting offset
      const originalSegments = mapFromCore(coreStateRaw).segments;
      let originalOffset = 0;
      for (const s of originalSegments) {
        if (s.id === dragState.id) break;
        originalOffset += s.sizeGb;
      }

      let newOffset = 0;
      for (const s of state.segments) {
        if (s.id === dragState.id) break;
        newOffset += s.sizeGb;
      }

      const deltaGb = newOffset - originalOffset;
      const deltaMib = Math.round(deltaGb * 1024);

      // Restore order first so UI returns to baseline before Tauri updates state
      restoreOrder(dragState.originalOrder);

      if (Math.abs(deltaMib) >= 1) {
        const target = draggedSegment.letter || (draggedSegment.index !== undefined && draggedSegment.index !== null ? `${draggedSegment.index}` : draggedSegment.id);
        const inputs = buildInputs({
          action: { Move: { target, delta_mib: deltaMib } }
        });

        enqueue({
          title: `Move ${labelFor(draggedSegment)} ${deltaMib > 0 ? "Right" : "Left"}`,
          details: `Move ${deltaMib > 0 ? "right" : "left"} by ${formatSize(Math.abs(deltaGb))}.`,
          status: "Pending Apply",
          inputs
        });
        addLog("info", `Queued move: ${labelFor(draggedSegment)}.`);
        updateComposedState();
      }
    } else {
      if (!dragState.moved) {
        const segment = state.segments[dragState.startIndex];
        if (segment && segment.kind !== "unallocated") {
          openResizeModal(segment);
        }
      }
    }

    dragState = null;
    document.body.classList.remove("is-dragging");
  }

  function openResizeModal(segment) {
    if (!modalEl || !resizeLeftEl || !resizeRightEl || !resizeShrinkLeftEl || !resizeShrinkRightEl) return;
    const index = findIndex(segment.id);
    const before = adjacentUnallocated(index, -1);
    const after = adjacentUnallocated(index, 1);
    const minSize = Math.max(0.01, segment.usedGb + 0.01);
    const shrinkMax = Math.max(0, segment.sizeGb - minSize);

    resizeState = {
      segmentId: segment.id,
      baseSize: segment.sizeGb,
      beforeIndex: before ? before.index : null,
      afterIndex: after ? after.index : null,
      beforeMax: before ? before.sizeGb : 0,
      afterMax: after ? after.sizeGb : 0,
      minSize,
      shrinkMax
    };

    if (modalTitleEl) {
      modalTitleEl.textContent = `Resize/Move ${labelFor(segment)}`;
    }

    resizeLeftEl.min = "0";
    resizeLeftEl.max = resizeState.beforeMax.toFixed(3);
    resizeLeftEl.value = "0";
    resizeLeftEl.disabled = resizeState.beforeMax <= 0;

    resizeRightEl.min = "0";
    resizeRightEl.max = resizeState.afterMax.toFixed(3);
    resizeRightEl.value = "0";
    resizeRightEl.disabled = resizeState.afterMax <= 0;

    resizeShrinkLeftEl.min = "0";
    resizeShrinkLeftEl.max = resizeState.shrinkMax.toFixed(3);
    resizeShrinkLeftEl.value = "0";
    resizeShrinkLeftEl.disabled = resizeState.shrinkMax <= 0;

    resizeShrinkRightEl.min = "0";
    resizeShrinkRightEl.max = resizeState.shrinkMax.toFixed(3);
    resizeShrinkRightEl.value = "0";
    resizeShrinkRightEl.disabled = resizeState.shrinkMax <= 0;

    if (resizeBeforeEl) {
      resizeBeforeEl.textContent = before ? `(available: ${formatSize(before.sizeGb)})` : "(available: 0.00 GB)";
    }
    if (resizeAfterEl) {
      resizeAfterEl.textContent = after ? `(available: ${formatSize(after.sizeGb)})` : "(available: 0.00 GB)";
    }

    if (resizeCurrentEl) {
      resizeCurrentEl.textContent = formatSize(segment.sizeGb);
    }
    if (resizeMinEl) {
      resizeMinEl.textContent = formatSize(minSize);
    }

    updateResizePreview();
    modalEl.classList.remove("hidden");
  }

  function closeResizeModal() {
    if (!modalEl) return;
    modalEl.classList.add("hidden");
    resizeState = null;
  }

  function updateResizePreview() {
    if (!resizeState) return;
    const left = parseFloat(resizeLeftEl.value || "0");
    const right = parseFloat(resizeRightEl.value || "0");
    const shrinkL = parseFloat(resizeShrinkLeftEl.value || "0");
    const shrinkR = parseFloat(resizeShrinkRightEl.value || "0");

    if (resizeLeftValueEl) resizeLeftValueEl.textContent = formatSize(left);
    if (resizeRightValueEl) resizeRightValueEl.textContent = formatSize(right);
    if (resizeShrinkLeftValueEl) resizeShrinkLeftValueEl.textContent = formatSize(shrinkL);
    if (resizeShrinkRightValueEl) resizeShrinkRightValueEl.textContent = formatSize(shrinkR);

    const nextSize = resizeState.baseSize + left + right - shrinkL - shrinkR;
    if (resizeNewEl) resizeNewEl.textContent = formatSize(nextSize);

    if (resizeApplyEl) {
      resizeApplyEl.disabled = left <= 0 && right <= 0 && shrinkL <= 0 && shrinkR <= 0;
    }
  }

  function applyResize() {
    if (!resizeState) return;
    
    let left = parseFloat(resizeLeftEl.value || "0");
    let right = parseFloat(resizeRightEl.value || "0");
    let shrinkL = parseFloat(resizeShrinkLeftEl.value || "0");
    let shrinkR = parseFloat(resizeShrinkRightEl.value || "0");

    if (left <= 0 && right <= 0 && shrinkL <= 0 && shrinkR <= 0) return;

    if ((shrinkL > 0 || shrinkR > 0) && (left > 0 || right > 0)) {
      alert("Choose either shrink or extend, not both at once.");
      return;
    }

    // Clamp values if they are close to the maximum available unallocated space to prevent rounding/step issues.
    if (left > 0) {
      if (left >= resizeState.beforeMax - 0.0009 || left > resizeState.beforeMax) {
        left = resizeState.beforeMax;
      }
    }
    if (right > 0) {
      if (right >= resizeState.afterMax - 0.0009 || right > resizeState.afterMax) {
        right = resizeState.afterMax;
      }
    }
    const totalShrink = shrinkL + shrinkR;
    if (totalShrink > 0) {
      if (totalShrink >= resizeState.shrinkMax - 0.0009 || totalShrink > resizeState.shrinkMax) {
        const scale = resizeState.shrinkMax / totalShrink;
        shrinkL = shrinkL * scale;
        shrinkR = shrinkR * scale;
      }
    }

    const size = resizeState.baseSize + left + right - shrinkL - shrinkR;
    const deltaBefore = shrinkL - left;
    const deltaSize = size - resizeState.baseSize;

    if (Math.abs(deltaBefore) <= 0.0009 && Math.abs(deltaSize) <= 0.0009) {
      closeResizeModal();
      return;
    }

    const segmentIndex = findIndex(resizeState.segmentId);
    const segment = state.segments[segmentIndex];
    if (!segment) return;

    if (segment.status && segment.status.toLowerCase() === "pending") {
      openInfoModal("Resize", ["Apply the pending create before resizing this partition."]);
      return;
    }
    
    const target = segment.letter || (segment.index !== undefined && segment.index !== null ? `${segment.index}` : null);
    if (!target) {
      openInfoModal("Resize", ["Unable to resolve a resize target for this partition."]);
      return;
    }
    
    let currentSize = resizeState.baseSize;
    
    if (deltaBefore > 0.0009) {
      // Shrank from left: Resize smaller first, then Move right by deltaBefore
      currentSize = size;
      const resizeInputs = buildInputs({ action: { Resize: { target, size_gb: roundGb(currentSize) } } });
      const moveInputs = buildInputs({ action: { Move: { target, delta_mib: Math.round(deltaBefore * 1024) } } });
      enqueue({
        title: `Shrink ${labelFor(segment)} Left`,
        details: `Shrink to ${formatSize(currentSize)} and shift right by ${formatSize(deltaBefore)}.`,
        status: "Pending Apply",
        inputs: [resizeInputs, moveInputs]
      });
    } else if (deltaBefore < -0.0009) {
      // Extended to left: Move left first by -deltaBefore (which is left), then Extend larger
      const moveInputs = buildInputs({ action: { Move: { target, delta_mib: Math.round(deltaBefore * 1024) } } });
      let inputsList = [moveInputs];
      let details = `Shift left by ${formatSize(-deltaBefore)}`;
      
      if (Math.abs(deltaSize) > 0.0009) {
        currentSize = size;
        const resizeInputs = buildInputs({ action: { Resize: { target, size_gb: roundGb(currentSize) } } });
        inputsList.push(resizeInputs);
        details += ` and extend size to ${formatSize(currentSize)}`;
      }
      
      enqueue({
        title: `Extend ${labelFor(segment)} Left`,
        details: `${details}.`,
        status: "Pending Apply",
        inputs: inputsList
      });
    } else {
      // No move, just resize (extend right or shrink right)
      if (Math.abs(deltaSize) > 0.0009) {
        currentSize = size;
        const resizeInputs = buildInputs({ action: { Resize: { target, size_gb: roundGb(currentSize) } } });
        enqueue({
          title: deltaSize > 0 ? `Extend ${labelFor(segment)}` : `Shrink ${labelFor(segment)}`,
          details: `${deltaSize > 0 ? "Extend" : "Shrink"} to ${formatSize(currentSize)}.`,
          status: "Pending Apply",
          inputs: resizeInputs
        });
      }
    }

    addLog("info", `Queued resize operations for: ${labelFor(segment)}.`);

    updateComposedState();
    closeResizeModal();
  }

  let moveState = null;

  function openMoveModal(segment) {
    if (!moveModalEl || !moveSliderEl) return;
    const index = findIndex(segment.id);
    const before = adjacentUnallocated(index, -1);
    const after = adjacentUnallocated(index, 1);

    moveState = {
      segmentId: segment.id,
      baseSize: segment.sizeGb,
      beforeMax: before ? before.sizeGb : 0,
      afterMax: after ? after.sizeGb : 0,
    };

    if (moveTitleEl) {
      moveTitleEl.textContent = `Move ${labelFor(segment)}`;
    }

    moveSliderEl.min = (-moveState.beforeMax).toFixed(3);
    moveSliderEl.max = moveState.afterMax.toFixed(3);
    moveSliderEl.value = "0";
    moveSliderEl.step = "0.001";
    moveSliderEl.disabled = moveState.beforeMax <= 0 && moveState.afterMax <= 0;

    updateMovePreview();
    moveModalEl.classList.remove("hidden");
  }

  function closeMoveModal() {
    if (!moveModalEl) return;
    moveModalEl.classList.add("hidden");
    moveState = null;
  }
  function updateMovePreview() {
    if (!moveState) return;
    const offset = parseFloat(moveSliderEl.value || "0");

    const beforeSize = Math.max(0, moveState.beforeMax + offset);
    const afterSize = Math.max(0, moveState.afterMax - offset);

    if (moveShiftValueEl) {
      if (offset < 0) {
        moveShiftValueEl.textContent = `Left by ${formatSize(-offset)}`;
      } else if (offset > 0) {
        moveShiftValueEl.textContent = `Right by ${formatSize(offset)}`;
      } else {
        moveShiftValueEl.textContent = "0.00 GB";
      }
    }

    if (moveBeforeSizeEl) moveBeforeSizeEl.textContent = formatSize(beforeSize);
    if (moveAfterSizeEl) moveAfterSizeEl.textContent = formatSize(afterSize);

    // Update static preview blocks
    const showBefore = beforeSize > 0.0009;
    const showAfter = afterSize > 0.0009;

    if (moveBlockBeforeEl) {
      moveBlockBeforeEl.style.display = showBefore ? "flex" : "none";
    }
    if (moveBlockAfterEl) {
      moveBlockAfterEl.style.display = showAfter ? "flex" : "none";
    }

    if (showBefore && showAfter) {
      const total = beforeSize + moveState.baseSize + afterSize;
      const wBefore = beforeSize / total;
      const wPart = moveState.baseSize / total;
      const wAfter = afterSize / total;
      const pctBefore = 15 + 55 * wBefore;
      const pctPart = 15 + 55 * wPart;
      const pctAfter = 15 + 55 * wAfter;

      if (moveBlockBeforeEl) moveBlockBeforeEl.style.width = `${pctBefore}%`;
      if (moveBlockPartEl) moveBlockPartEl.style.width = `${pctPart}%`;
      if (moveBlockAfterEl) moveBlockAfterEl.style.width = `${pctAfter}%`;
    } else if (showBefore) {
      const total = beforeSize + moveState.baseSize;
      const wBefore = beforeSize / total;
      const wPart = moveState.baseSize / total;
      const pctBefore = 20 + 60 * wBefore;
      const pctPart = 20 + 60 * wPart;

      if (moveBlockBeforeEl) moveBlockBeforeEl.style.width = `${pctBefore}%`;
      if (moveBlockPartEl) moveBlockPartEl.style.width = `${pctPart}%`;
    } else if (showAfter) {
      const total = moveState.baseSize + afterSize;
      const wPart = moveState.baseSize / total;
      const wAfter = afterSize / total;
      const pctPart = 20 + 60 * wPart;
      const pctAfter = 20 + 60 * wAfter;

      if (moveBlockPartEl) moveBlockPartEl.style.width = `${pctPart}%`;
      if (moveBlockAfterEl) moveBlockAfterEl.style.width = `${pctAfter}%`;
    } else {
      if (moveBlockPartEl) moveBlockPartEl.style.width = "100%";
    }

    if (moveBlockBeforeSizeEl && showBefore) {
      moveBlockBeforeSizeEl.textContent = formatSize(beforeSize);
    }
    if (moveBlockAfterSizeEl && showAfter) {
      moveBlockAfterSizeEl.textContent = formatSize(afterSize);
    }

    if (moveBlockPartEl) {
      const segment = state.segments.find(s => s.id === moveState.segmentId);
      
      // Remove existing tone classes
      moveBlockPartEl.classList.remove("tone-primary", "tone-secondary", "tone-tertiary", "tone-primary-container", "tone-surface");
      const toneClass = segment ? `tone-${segment.tone}` : "tone-primary";
      moveBlockPartEl.classList.add(toneClass);

      if (moveBlockPartLabelEl) {
        moveBlockPartLabelEl.textContent = segment ? labelFor(segment) : "Partition";
      }
      if (moveBlockPartSizeEl) {
        moveBlockPartSizeEl.textContent = formatSize(moveState.baseSize);
      }
    }

    if (moveApplyEl) {
      moveApplyEl.disabled = Math.abs(offset) <= 0.0009;
    }
  }

  function applyMove() {
    if (!moveState) return;
    let offset = parseFloat(moveSliderEl.value || "0");
    if (Math.abs(offset) <= 0.0009) return;

    if (offset < 0) {
      if (offset <= -moveState.beforeMax + 0.0009) {
        offset = -moveState.beforeMax;
      }
    } else if (offset > 0) {
      if (offset >= moveState.afterMax - 0.0009) {
        offset = moveState.afterMax;
      }
    }

    const segmentIndex = findIndex(moveState.segmentId);
    const segment = state.segments[segmentIndex];
    if (!segment) return;

    if (segment.status && segment.status.toLowerCase() === "pending") {
      openInfoModal("Move", ["Apply the pending create before moving this partition."]);
      return;
    }

    const target = segment.letter || (segment.index !== undefined && segment.index !== null ? `${segment.index}` : null);
    if (!target) {
      openInfoModal("Move", ["Unable to resolve a move target for this partition."]);
      return;
    }

    const deltaMib = Math.round(offset * 1024);
    const moveInputs = buildInputs({
      action: { Move: { target, delta_mib: deltaMib } }
    });

    enqueue({
      title: `Move ${labelFor(segment)} ${offset > 0 ? "Right" : "Left"}`,
      details: `Move ${offset > 0 ? "right" : "left"} by ${formatSize(Math.abs(offset))}.`,
      status: "Pending Apply",
      inputs: moveInputs
    });

    addLog("info", `Queued move operation for: ${labelFor(segment)}.`);
    updateComposedState();
    closeMoveModal();
  }

  function adjacentUnallocated(index, direction) {
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= state.segments.length) return null;
    const segment = state.segments[targetIndex];
    if (!segment || segment.kind !== "unallocated") return null;
    return { index: targetIndex, sizeGb: segment.sizeGb };
  }

  function coalesceUnallocated() {
    const merged = [];
    state.segments.forEach((segment) => {
      if (segment.kind === "unallocated") {
        if (segment.sizeGb < MIN_UNALLOCATED_GB) return;
        const last = merged[merged.length - 1];
        if (last && last.kind === "unallocated") {
          last.sizeGb = roundGb(last.sizeGb + segment.sizeGb);
          return;
        }
      }
      merged.push(segment);
    });
    state.segments = merged;
    if (selectedId && !state.segments.some((segment) => segment.id === selectedId)) {
      selectedId = null;
    }
  }

  function enqueue(item) {
    const id = Date.now() + Math.floor(Math.random() * 1000);
    state.queue.push({ id, ...item });
    renderQueue();
    saveQueue();
    return id;
  }

  function addLog(level, message) {
    console.log(`[${level}] ${message}`);
  }

  function saveQueue() {
    if (!window.localStorage) return;
    try {
      localStorage.setItem(SAVED_QUEUE_KEY, JSON.stringify(state.queue));
    } catch (err) {
      console.warn("Failed to persist queue.", err);
    }
    updateRestoreBadge();
  }

  function getSavedQueue() {
    if (!window.localStorage) return [];
    try {
      const raw = localStorage.getItem(SAVED_QUEUE_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed : [];
    } catch (err) {
      console.warn("Failed to read saved queue.", err);
      return [];
    }
  }

  function updateRestoreBadge() {
    if (!restoreBtn) return;
    restoreBtn.title = "Restore partition layout snapshot";
  }

  function setButtonLoading(button, loading) {
    if (!button) return;
    if (loading) {
      button.classList.add("is-loading");
      button.disabled = true;
      button.setAttribute("aria-busy", "true");
    } else {
      button.classList.remove("is-loading");
      button.disabled = false;
      button.removeAttribute("aria-busy");
    }
  }

  function setStartupLoading(loading) {
    if (!startupLoadingEl) return;
    if (loading) {
      startupLoadingEl.classList.remove("hidden");
    } else {
      startupLoadingEl.classList.add("hidden");
    }
  }

  async function updateComposedState() {
    if (!invoke) return;
    try {
      const queueInputs = [];
      state.queue.forEach((item) => {
        if (item.inputs) {
          if (Array.isArray(item.inputs)) {
            queueInputs.push(...item.inputs);
          } else {
            queueInputs.push(item.inputs);
          }
        }
      });

      const composedState = await invoke("compose_plan_state", {
        currentState: coreStateRaw,
        current_state: coreStateRaw,
        queue: queueInputs
      });
      
      if (!composedState || !composedState.disks || composedState.disks.length === 0) {
        console.warn("compose_plan_state returned empty or invalid state");
      }
      
      const mapped = mapFromCore(composedState);
      state.segments = mapped.segments;
      render();
    } catch (err) {
      console.error("Failed to compose plan state:", err);
      alert("Failed to compose plan state: " + err);
    }
  }

  async function refreshState({ logMessage, button, silent } = {}) {
    if (!invoke) return;
    if (button) setButtonLoading(button, true);
    try {
      const coreState = await invoke("get_state");
      coreStateRaw = coreState;
      state = mapFromCore(coreState);
      render();
      if (logMessage && !silent) addLog("info", logMessage);
    } catch (err) {
      console.error("Refresh failed:", err);
      if (!silent) addLog("error", "Refresh failed.");
    } finally {
      if (button) setButtonLoading(button, false);
    }
  }

  function neighborLabel(index) {
    if (index === 0) return "to the start";
    if (index === state.segments.length - 1) return "to the end";
    const left = state.segments[index - 1];
    return `after ${labelFor(left)}`;
  }

  function labelFor(segment) {
    return segment.letter ? `${segment.label} (${segment.letter}:)` : segment.label;
  }

  function buildInputs(overrides) {
    const diskIdx = coreStateRaw && coreStateRaw.disks && selectedDiskIndex < coreStateRaw.disks.length ? selectedDiskIndex : 0;
    const diskId = coreStateRaw && coreStateRaw.disks && coreStateRaw.disks[diskIdx] && coreStateRaw.disks[diskIdx].id
      ? coreStateRaw.disks[diskIdx].id
      : "\\\\.\\PhysicalDrive0";

    return {
      action: null,
      disk: diskId,
      test_vhd: null,
      restore_gpt: null,
      dry_run: true,
      move_unallocated_mib: null,
      move_sectors: null,
      extend_index: null,
      move_index: null,
      free_before_index: null,
      free_after_index: null,
      source_unalloc_lba: null,
      chunk_mib: 64,
      keep_traces: false,
      wipe_old_data: false,
      system_letter: "C",
      data_letter: "D",
      out_dir: "",
      force_resume: false,
      operation_type: null,
      target_lba: null,
      target_sectors: null,
      format_fs: null,
      ...overrides
    };
  }

  function arraysEqual(left, right) {
    if (left.length !== right.length) return false;
    for (let i = 0; i < left.length; i += 1) {
      if (left[i] !== right[i]) return false;
    }
    return true;
  }

  function restoreOrder(order) {
    if (!order || order.length === 0) return;
    const byId = new Map(state.segments.map((segment) => [segment.id, segment]));
    state.segments = order.map((id) => byId.get(id)).filter(Boolean);
    renderDiskMap();
    renderTable();
  }

  function getSelectedSegment() {
    if (!selectedId) return null;
    return state.segments.find((segment) => segment.id === selectedId) || null;
  }

  function findIndex(id) {
    return state.segments.findIndex((segment) => segment.id === id);
  }

  function indexFromX(x, width) {
    let cursor = 0;
    for (let i = 0; i < state.segments.length; i += 1) {
      const seg = state.segments[i];
      const segWidth = (seg.sizeGb / state.totalSizeGb) * width;
      if (x < cursor + segWidth / 2) return i;
      cursor += segWidth;
    }
    return state.segments.length - 1;
  }

  function roundGb(value) {
    return Math.round(value * 1000) / 1000;
  }

  function formatSize(valueGb) {
    if (valueGb < 1) {
      return `${Math.round(valueGb * 1024)} MB`;
    }
    return `${valueGb.toFixed(2)} GB`;
  }
})();
