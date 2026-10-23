type canvasSize = {
  width: float,
  height: float,
  scale: float,
  scaledWidth: float,
  scaledHeight: float,
  maxSceneWidth: float,
  frameToPxRatio: float,
  pxToFrameRatio: float,
  viewportOffset: float,
  timelineMarginLeft: int,
  timelineMarginRight: int,
}

// Make sure to not change this from ints to float to enable preval of calculations
@inline
let timeline_margin_x = 64
@inline
let timeline_margin_y = 64
@inline
let scene_height_size = 120
@inline
let timeline_scenes_start_y = 24

let audio_height = scene_height_size / 2

module Canvas = Webapi.Canvas
module Canvas2d = Webapi.Canvas.Canvas2d

let useCanvasScale = (elementRef: React.ref<'a>, size) => {
  React.useEffect1(() => {
    elementRef.current
    ->Js.Nullable.toOption
    ->Belt.Option.forEach(canvasElement => {
      let ctx = Webapi.Canvas.CanvasElement.getContext2d(canvasElement)

      canvasElement->Canvas.CanvasElement.setHeight(
        size.scaledHeight->Js.Math.floor->Belt.Float.toInt,
      )
      canvasElement->Canvas.CanvasElement.setWidth(
        size.scaledWidth->Js.Math.floor->Belt.Float.toInt,
      )

      ctx->Canvas2d.scale(~x=size.scale, ~y=size.scale)
    })

    None
  }, [size])
}

let calculateTimelineMargins = (
  viewportOffset: float,
  ~videoTotalWidth: float,
  ~visibleWidth: float,
) => {
  // Calculate if we're at the video start or end
  let atVideoStart = viewportOffset <= 0.0
  let atVideoEnd = viewportOffset +. visibleWidth >= videoTotalWidth

  // Calculate left margin when at video start
  let leftMargin = if atVideoStart {
    // At video start: show margin proportional to how much video start is visible
    let visibleStartPortion = Js.Math.min(32.0, -.viewportOffset +. 32.0)
    Js.Math.max(0.0, visibleStartPortion)->Belt.Float.toInt
  } else {
    0
  }

  // Calculate right margin when at video end
  let rightMargin = if atVideoEnd {
    // At video end: show margin proportional to how much past the end we can see
    let pastEndPortion = viewportOffset +. visibleWidth -. videoTotalWidth
    let visibleEndPortion = Js.Math.min(32.0, pastEndPortion +. 32.0)
    Js.Math.max(0.0, visibleEndPortion)->Belt.Float.toInt
  } else {
    0
  }

  (leftMargin, rightMargin)
}

// Backward compatibility function for left margin only
let calculateTimelineMargin = (
  viewportOffset: float,
  ~videoTotalWidth: float,
  ~visibleWidth: float,
) => {
  let (leftMargin, _) = calculateTimelineMargins(viewportOffset, ~videoTotalWidth, ~visibleWidth)
  leftMargin
}

let frameToX = (frame, size: canvasSize) =>
  Belt.Float.fromInt(frame) *. size.frameToPxRatio +.
  Belt.Float.fromInt(size.timelineMarginLeft) -.
  size.viewportOffset
