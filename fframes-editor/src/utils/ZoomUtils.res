let calculateLogarithmicZoom = (currentZoom: float, deltaY: float, ~sensitivity: float=1.0, ()) => {
  // Use logarithmic scaling for more natural zoom feel
  let logZoom = Js.Math.log(currentZoom)
  let zoomSpeed = sensitivity *. 0.1
  let newLogZoom = logZoom -. deltaY *. zoomSpeed *. 0.01

  // Convert back to linear scale
  let newZoom = Js.Math.exp(newLogZoom)

  // Apply zoom sensitivity scaling - slower at higher zoom levels
  let adaptiveSensitivity = 1.0 /. (1.0 +. currentZoom *. 0.1)
  let adjustedNewZoom = currentZoom +. (newZoom -. currentZoom) *. adaptiveSensitivity

  adjustedNewZoom
}

let calculateZoomFactorFromDelta = (
  deltaY: float,
  ~currentZoom: float,
  ~sensitivity: float=1.0,
  (),
) => {
  // Base zoom factor depends on current zoom level for more natural feel
  let baseZoomFactor = if deltaY > 0.0 {
    0.85 +. currentZoom *. 0.02
  } else {
    // Slower zoom out at higher levels

    1.15 -. currentZoom *. 0.02 // Slower zoom in at higher levels
  }

  // Apply sensitivity scaling
  let zoomFactor = if deltaY > 0.0 {
    1.0 -. (1.0 -. baseZoomFactor) *. sensitivity
  } else {
    1.0 +. (baseZoomFactor -. 1.0) *. sensitivity
  }

  zoomFactor
}

let getZoomSensitivity = (~ctrlKey: bool, ~shiftKey: bool) => {
  if ctrlKey {
    0.3 // Fine zoom with Ctrl
  } else if shiftKey {
    2.0 // Coarse zoom with Shift
  } else {
    1.0 // Normal zoom
  }
}

// Calculate new viewport offset to keep current frame (seek bar) position fixed during zoom
let calculateViewportOffsetForCurrentFrameZoom = (
  ~currentFrame: int,
  ~currentZoom: float,
  ~newZoom: float,
  ~currentViewportOffset: float,
  ~maxSceneWidth: float,
  ~totalFrames: int,
  ~timelineMarginLeft: float,
) => {
  // Safety checks to prevent infinite loops
  if totalFrames <= 0 || maxSceneWidth <= 0.0 || currentZoom <= 0.0 || newZoom <= 0.0 {
    currentViewportOffset // Return current offset if inputs are invalid
  } else {
    // Simple, precise calculation to keep frame at exact same pixel position
    let currentFrameFloat = currentFrame->Js.Int.toFloat
    let totalFramesFloat = totalFrames->Js.Int.toFloat

    // Calculate base pixel ratios - this should match the Timeline calculation exactly
    let basePixelRatio = maxSceneWidth /. totalFramesFloat
    let oldFrameToPxRatio = basePixelRatio *. currentZoom
    let newFrameToPxRatio = basePixelRatio *. newZoom

    // Current frame's screen position using frameToX formula: frame * frameToPxRatio + timelineMarginLeft - viewportOffset
    let currentFrameScreenX =
      currentFrameFloat *. oldFrameToPxRatio +. timelineMarginLeft -. currentViewportOffset

    // Calculate the new viewport offset to keep the frame at the same screen position
    // We want: currentFrame * newFrameToPxRatio + timelineMarginLeft - newViewportOffset = currentFrameScreenX
    // Solving: newViewportOffset = currentFrame * newFrameToPxRatio + timelineMarginLeft - currentFrameScreenX
    let newViewportOffset =
      currentFrameFloat *. newFrameToPxRatio +. timelineMarginLeft -. currentFrameScreenX

    // Safety check for NaN values
    if Js.Float.isNaN(newViewportOffset) {
      currentViewportOffset
    } else {
      // Apply minimal bounds constraints
      let totalContentWidth = totalFramesFloat *. newFrameToPxRatio
      let maxOffset = Js.Math.max(0.0, totalContentWidth -. maxSceneWidth)

      // Apply loose bounds - allow some overflow to maintain frame position
      let minBound = -.maxSceneWidth *. 0.1 // Allow 10% left overflow
      let maxBound = maxOffset +. maxSceneWidth *. 0.1 // Allow 10% right overflow

      Js.Math.max(minBound, Js.Math.min(newViewportOffset, maxBound))
    }
  }
}
