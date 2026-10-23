module type Observable = {
  type state
  type action

  let initial: state

  let reducer: (state, action) => state
}

type unlisten = unit => unit

module type Observer = (ObservableT: Observable) =>
{
  type t = ObservableT.state
  type action = ObservableT.action
  type listener = t => unit

  let dispatch: action => unit
  let get: unit => t
  let subscribe: (listener, unit) => unit
  let useObservable: unit => t
}

module type PubsubInit = {
  type t
  let initial: t
}

module Pubsub = (Init: PubsubInit) => {
  type listener = Init.t => unit
  type listenerId = {id: int, listener: listener}

  let mutableState = ref(Init.initial)
  let listeners: array<listenerId> = []

  let get = () => mutableState.contents

  let set = newState => {
    mutableState := newState
    listeners->Js.Array.forEach(({listener}) => listener(mutableState.contents))
  }

  let nextId = ref(0)
  let subscribe = listener => {
    let id = nextId.contents
    nextId := id + 1

    listeners->Js.Array.push({id: id, listener: listener})->ignore

    () => {
      let index = listeners->Js.Array.findIndex(listener => listener.id === id)

      if index >= 0 {
        listeners->Utils.Array.removeInPlace(~index)->ignore
      }
    }
  }

  let useObservable = () => {
    let (_, forceUpdate) = React.useReducer((x, _) => x + 1, 0)

    React.useEffect0(() => {
      Some(subscribe(forceUpdate))
    })

    get()
  }
}

module MakeObserver: Observer = (Observable: Observable) => {
  type t = Observable.state
  type action = Observable.action

  module ObserverPubSubState = {
    type t = Observable.state
    let initial = Observable.initial
  }

  include Pubsub(ObserverPubSubState)

  let dispatch = action => {
    set(Observable.reducer(mutableState.contents, action))
  }
}
