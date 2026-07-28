#include "Ksa64GlobalViewerOverlay.h"

#include "Ksa64GlobalViewerPolicy.h"
#include "Ksa64GlobalViewerSubsystem.h"
#include "Ksa64LiveMissionSubsystem.h"

#include "InputCoreTypes.h"
#include "Styling/CoreStyle.h"
#include "Widgets/Input/SButton.h"
#include "Widgets/Layout/SBorder.h"
#include "Widgets/Layout/SBox.h"
#include "Widgets/SOverlay.h"
#include "Widgets/SBoxPanel.h"
#include "Widgets/Text/STextBlock.h"

namespace
{
const FLinearColor Cyan(0.14f, 0.83f, 0.95f, 1.0f);
const FLinearColor Amber(1.0f, 0.66f, 0.18f, 1.0f);
const FLinearColor Green(0.31f, 0.93f, 0.57f, 1.0f);
const FLinearColor Muted(0.58f, 0.68f, 0.76f, 1.0f);
const FLinearColor Panel(0.012f, 0.025f, 0.043f, 0.88f);

TSharedRef<STextBlock> Label(
    TAttribute<FText> Text,
    int32 Size,
    const FLinearColor& Color)
{
    return SNew(STextBlock)
        .Text(Text)
        .ColorAndOpacity(Color)
        .Font(FCoreStyle::GetDefaultFontStyle(TEXT("Regular"), Size))
        .AutoWrapText(true);
}
}

void SKsa64GlobalViewerOverlay::Construct(const FArguments& Args)
{
    Subsystem = Args._Subsystem;
    SetVisibility(EVisibility::SelfHitTestInvisible);
    ChildSlot
    [
        SNew(SOverlay)
        + SOverlay::Slot()
        .HAlign(HAlign_Fill)
        .VAlign(VAlign_Top)
        .Padding(FMargin(18.0f))
        [
            BuildTopBar()
        ]
        + SOverlay::Slot()
        .HAlign(HAlign_Right)
        .VAlign(VAlign_Center)
        .Padding(FMargin(18.0f, 96.0f, 18.0f, 90.0f))
        [
            SNew(SBox)
            .WidthOverride(390.0f)
            .Visibility(this, &SKsa64GlobalViewerOverlay::EngineeringVisibility)
            [
                BuildEngineeringPanel()
            ]
        ]
        + SOverlay::Slot()
        .HAlign(HAlign_Fill)
        .VAlign(VAlign_Bottom)
        .Padding(FMargin(18.0f))
        [
            SNew(SBox)
            .Visibility(this, &SKsa64GlobalViewerOverlay::BottomVisibility)
            [
                BuildBottomBar()
            ]
        ]
    ];
}

TSharedRef<SWidget> SKsa64GlobalViewerOverlay::BuildTopBar()
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(Panel)
        .Padding(FMargin(14.0f, 10.0f))
        [
            SNew(SHorizontalBox)
            + SHorizontalBox::Slot()
            .FillWidth(1.0f)
            .VAlign(VAlign_Center)
            [
                SNew(SVerticalBox)
                + SVerticalBox::Slot().AutoHeight()
                [
                    Label(
                        FText::FromString(TEXT("KSA64  /  GLOBAL MISSION VIEWER")),
                        17,
                        FLinearColor::White)
                ]
                + SVerticalBox::Slot().AutoHeight()
                [
                    Label(
                        TAttribute<FText>::CreateLambda([Weak = Subsystem]()
                        {
                            return Weak.IsValid()
                                ? Weak->GetStatusText()
                                : FText::FromString(TEXT("VIEWER UNAVAILABLE"));
                        }),
                        10,
                        Cyan)
                ]
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    FText::FromString(TEXT("BEGIN GUIDED OPS")),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnStart))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    FText::FromString(TEXT("LAYOUT")),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnLayout))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    FText::FromString(TEXT("CAMERA")),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnCamera))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    FText::FromString(TEXT("AUTO")),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnAutomatic))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    TAttribute<FText>::CreateSP(
                        this,
                        &SKsa64GlobalViewerOverlay::OperationsButtonText),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnOperations))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    TAttribute<FText>::CreateSP(
                        this,
                        &SKsa64GlobalViewerOverlay::TruthButtonText),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnTruth),
                    TAttribute<bool>::CreateLambda([Weak = Subsystem]()
                    {
                        return Weak.IsValid() && Weak->CanShowTruth();
                    }))
            ]
        ];
}

TSharedRef<SWidget> SKsa64GlobalViewerOverlay::BuildEngineeringPanel()
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(Panel)
        .Padding(FMargin(16.0f))
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot().AutoHeight()
            [
                Label(FText::FromString(TEXT("DISPLAY AUTHORITY")), 11, Amber)
            ]
            + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 9.0f)
            [
                Label(
                    TAttribute<FText>::CreateLambda([Weak = Subsystem]()
                    {
                        if (!Weak.IsValid()) return FText::GetEmpty();
                        const FKsa64GlobalSemanticState& State =
                            Weak->GetSemanticState();
                        return FText::FromString(FString::Printf(
                            TEXT("%s\n%s\n\nRELEASE  %u\nTIME Q16  %u\n"
                                 "FRAME  %u\nSEGMENT  %u\n\n"
                                 "ORIGIN Q12 KM\n%lld  %lld  %lld"),
                            *State.RoleLabel,
                            *State.FrameLabel,
                            State.ReleaseEpoch,
                            State.MissionTimeQ16,
                            State.FrameIdentity,
                            State.SegmentIdentity,
                            static_cast<long long>(State.DisplayOriginQ12Km[0]),
                            static_cast<long long>(State.DisplayOriginQ12Km[1]),
                            static_cast<long long>(State.DisplayOriginQ12Km[2])));
                    }),
                    10,
                    FLinearColor::White)
            ]
            + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 10.0f, 0.0f, 0.0f)
            [
                Label(
                    TAttribute<FText>::CreateLambda([Weak = Subsystem]()
                    {
                        return Weak.IsValid()
                            ? Weak->GetSourceLegendText()
                            : FText::GetEmpty();
                    }),
                    9,
                    Muted)
            ]
        ];
}

TSharedRef<SWidget> SKsa64GlobalViewerOverlay::BuildBottomBar()
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(Panel)
        .Padding(FMargin(14.0f, 8.0f))
        [
            SNew(SHorizontalBox)
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    TAttribute<FText>::CreateSP(
                        this,
                        &SKsa64GlobalViewerOverlay::PauseButtonText),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnPauseResume))
            ]
            + SHorizontalBox::Slot().AutoWidth().Padding(4.0f, 0.0f)
            [
                Button(
                    FText::FromString(TEXT("STEP +1")),
                    FOnClicked::CreateSP(this, &SKsa64GlobalViewerOverlay::OnStep))
            ]
            + SHorizontalBox::Slot().FillWidth(1.0f).Padding(14.0f, 0.0f)
            .VAlign(VAlign_Center)
            [
                Label(
                    TAttribute<FText>::CreateLambda([Weak = Subsystem]()
                    {
                        if (!Weak.IsValid()) return FText::GetEmpty();
                        const FKsa64GlobalSemanticState& State =
                            Weak->GetSemanticState();
                        return FText::FromString(FString::Printf(
                            TEXT("%s  ·  %s  ·  %s  ·  %s"),
                            *Weak->GetLayoutText().ToString(),
                            *Weak->GetCameraText().ToString(),
                            *State.SourceLabel,
                            *State.DispositionLabel));
                    }),
                    10,
                    Green)
            ]
        ];
}

TSharedRef<SWidget> SKsa64GlobalViewerOverlay::Button(
    TAttribute<FText> Text,
    const FOnClicked& Clicked,
    TAttribute<bool> Enabled) const
{
    const TSharedRef<SButton> Result = SNew(SButton)
        .ButtonColorAndOpacity(FLinearColor(0.025f, 0.12f, 0.16f, 1.0f))
        .ForegroundColor(Cyan)
        .ContentPadding(FMargin(10.0f, 6.0f))
        .IsEnabled(Enabled)
        .ToolTipText(Text)
        .OnClicked(Clicked)
        [
            SNew(STextBlock)
            .Text(Text)
            .Font(FCoreStyle::GetDefaultFontStyle(TEXT("Bold"), 9))
        ];
    Result->SetAccessibleBehavior(EAccessibleBehavior::Custom, Text);
    return Result;
}

FReply SKsa64GlobalViewerOverlay::OnStart()
{
    if (Subsystem.IsValid() && Subsystem->GetGameInstance() != nullptr)
    {
        if (UKsa64LiveMissionSubsystem* Operations =
                Subsystem->GetGameInstance()->GetSubsystem<UKsa64LiveMissionSubsystem>())
        {
            Operations->StartGuidedOperations();
        }
    }
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnLayout()
{
    if (Subsystem.IsValid()) Subsystem->CycleLayout();
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnCamera()
{
    if (Subsystem.IsValid()) Subsystem->CycleCamera();
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnAutomatic()
{
    if (Subsystem.IsValid()) Subsystem->ResumeAutomaticDirector();
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnOperations()
{
    if (Subsystem.IsValid()) Subsystem->ToggleOperationsDesk();
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnTruth()
{
    if (Subsystem.IsValid()) Subsystem->ToggleTruth();
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnPauseResume()
{
    if (Subsystem.IsValid() && Subsystem->GetGameInstance() != nullptr)
    {
        if (UKsa64LiveMissionSubsystem* Operations =
                Subsystem->GetGameInstance()->GetSubsystem<UKsa64LiveMissionSubsystem>())
        {
            if (Operations->GetViewModel().PresentationPace
                == EKsa64OperationsPace::Paused)
            {
                Operations->ResumeRealtime();
            }
            else
            {
                Operations->PausePresentation();
            }
        }
    }
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnStep()
{
    if (Subsystem.IsValid() && Subsystem->GetGameInstance() != nullptr)
    {
        if (UKsa64LiveMissionSubsystem* Operations =
                Subsystem->GetGameInstance()->GetSubsystem<UKsa64LiveMissionSubsystem>())
        {
            Operations->StepOneRelease();
        }
    }
    return FReply::Handled();
}

FReply SKsa64GlobalViewerOverlay::OnKeyDown(
    const FGeometry& MyGeometry,
    const FKeyEvent& InKeyEvent)
{
    const FKey Key = InKeyEvent.GetKey();
    if (Key == EKeys::V) return OnLayout();
    if (Key == EKeys::C) return OnCamera();
    if (Key == EKeys::A) return OnAutomatic();
    if (Key == EKeys::O) return OnOperations();
    if (Key == EKeys::T) return OnTruth();
    if (Key == EKeys::SpaceBar) return OnPauseResume();
    if (Key == EKeys::Period) return OnStep();
    return SCompoundWidget::OnKeyDown(MyGeometry, InKeyEvent);
}

EVisibility SKsa64GlobalViewerOverlay::EngineeringVisibility() const
{
    if (!Subsystem.IsValid())
    {
        return EVisibility::Collapsed;
    }
    return Subsystem->GetSemanticState().Layout
            == EKsa64GlobalViewerLayout::CinematicFullscreen
        ? EVisibility::Collapsed
        : EVisibility::SelfHitTestInvisible;
}

EVisibility SKsa64GlobalViewerOverlay::BottomVisibility() const
{
    return EngineeringVisibility();
}

FText SKsa64GlobalViewerOverlay::TruthButtonText() const
{
    if (!Subsystem.IsValid() || !Subsystem->CanShowTruth())
    {
        return FText::FromString(TEXT("TRUTH UNAVAILABLE"));
    }
    return FText::FromString(
        Subsystem->GetSemanticState().bTruthVisible
            ? TEXT("HIDE SIM TRUTH")
            : TEXT("SHOW SIM TRUTH"));
}

FText SKsa64GlobalViewerOverlay::OperationsButtonText() const
{
    return FText::FromString(
        Subsystem.IsValid()
            && Subsystem->GetSemanticState().bOperationsDeskVisible
            ? TEXT("CLOSE OPS DESK")
            : TEXT("OPEN OPS DESK"));
}

FText SKsa64GlobalViewerOverlay::PauseButtonText() const
{
    if (!Subsystem.IsValid() || Subsystem->GetGameInstance() == nullptr)
    {
        return FText::FromString(TEXT("PAUSE"));
    }
    if (const UKsa64LiveMissionSubsystem* Operations =
            Subsystem->GetGameInstance()->GetSubsystem<UKsa64LiveMissionSubsystem>())
    {
        return FText::FromString(
            Operations->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused
                ? TEXT("RESUME")
                : TEXT("PAUSE"));
    }
    return FText::FromString(TEXT("PAUSE"));
}
