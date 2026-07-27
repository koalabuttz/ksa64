#include "Ksa64OperationsDashboard.h"

#include "Ksa64LiveMissionSubsystem.h"

#include "Rendering/DrawElements.h"
#include "InputCoreTypes.h"
#include "Styling/CoreStyle.h"
#include "Widgets/Images/SImage.h"
#include "Widgets/Input/SButton.h"
#include "Widgets/Layout/SBorder.h"
#include "Widgets/Layout/SBox.h"
#include "Widgets/Layout/SGridPanel.h"
#include "Widgets/Layout/SSpacer.h"
#include "Widgets/Layout/SSplitter.h"
#include "Widgets/Layout/SUniformGridPanel.h"
#include "Widgets/SBoxPanel.h"
#include "Widgets/Text/STextBlock.h"

namespace
{
const FLinearColor Background(0.012f, 0.020f, 0.035f, 1.0f);
const FLinearColor PanelDark(0.025f, 0.043f, 0.065f, 0.98f);
const FLinearColor PanelHighContrast(0.005f, 0.008f, 0.012f, 1.0f);
const FLinearColor Cyan(0.14f, 0.83f, 0.95f, 1.0f);
const FLinearColor Amber(1.0f, 0.66f, 0.18f, 1.0f);
const FLinearColor Green(0.31f, 0.93f, 0.57f, 1.0f);
const FLinearColor Red(1.0f, 0.27f, 0.25f, 1.0f);
const FLinearColor Muted(0.47f, 0.58f, 0.68f, 1.0f);
const FLinearColor White(0.91f, 0.95f, 0.98f, 1.0f);
const FMargin PanelPadding(14.0f, 11.0f);
}

void SKsa64OperationsPlot::Construct(const FArguments& Args)
{
    Subsystem = Args._Subsystem;
}

FVector2D SKsa64OperationsPlot::ComputeDesiredSize(float LayoutScaleMultiplier) const
{
    return FVector2D(620.0f, 260.0f);
}

int32 SKsa64OperationsPlot::OnPaint(
    const FPaintArgs& Args,
    const FGeometry& AllottedGeometry,
    const FSlateRect& MyCullingRect,
    FSlateWindowElementList& OutDrawElements,
    int32 LayerId,
    const FWidgetStyle& InWidgetStyle,
    bool bParentEnabled) const
{
    const FSlateBrush* WhiteBrush = FCoreStyle::Get().GetBrush(TEXT("WhiteBrush"));
    FSlateDrawElement::MakeBox(
        OutDrawElements, LayerId, AllottedGeometry.ToPaintGeometry(), WhiteBrush,
        ESlateDrawEffect::None, FLinearColor(0.008f, 0.017f, 0.027f, 1.0f));

    const FVector2D Size = AllottedGeometry.GetLocalSize();
    for (int32 Line = 1; Line < 8; ++Line)
    {
        const float X = Size.X * static_cast<float>(Line) / 8.0f;
        TArray<FVector2D> Points{FVector2D(X, 0.0f), FVector2D(X, Size.Y)};
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 1, AllottedGeometry.ToPaintGeometry(), Points, ESlateDrawEffect::None, FLinearColor(0.08f, 0.15f, 0.20f, 0.75f), true, 1.0f);
    }
    for (int32 Line = 1; Line < 4; ++Line)
    {
        const float Y = Size.Y * static_cast<float>(Line) / 4.0f;
        TArray<FVector2D> Points{FVector2D(0.0f, Y), FVector2D(Size.X, Y)};
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 1, AllottedGeometry.ToPaintGeometry(), Points, ESlateDrawEffect::None, FLinearColor(0.08f, 0.15f, 0.20f, 0.75f), true, 1.0f);
    }
    if (!Subsystem.IsValid()) return LayerId + 1;

    const TArray<FKsa64OperationsReleasePoint>& History = Subsystem->GetReleaseHistory();
    const TArray<FKsa64OperationsPredictionPoint>& Prediction = Subsystem->GetPredictionPath();
    int32 MinHeight = MAX_int32;
    int32 MaxHeight = MIN_int32;
    uint32 MinRelease = MAX_uint32;
    uint32 MaxRelease = 0;
    for (const FKsa64OperationsReleasePoint& Point : History)
    {
        if (!Point.bHasPosition) continue;
        MinRelease = FMath::Min(MinRelease, Point.ReleaseEpoch);
        MaxRelease = FMath::Max(MaxRelease, Point.ReleaseEpoch);
        MinHeight = FMath::Min(MinHeight, Point.PositionQ12[2]);
        MaxHeight = FMath::Max(MaxHeight, Point.PositionQ12[2]);
        if (Point.bHasGroundEstimate)
        {
            MinHeight = FMath::Min(MinHeight, Point.GroundPositionQ12[2]);
            MaxHeight = FMath::Max(MaxHeight, Point.GroundPositionQ12[2]);
        }
    }
    for (const FKsa64OperationsPredictionPoint& Point : Prediction)
    {
        MinRelease = FMath::Min(MinRelease, Point.ReleaseEpoch);
        MaxRelease = FMath::Max(MaxRelease, Point.ReleaseEpoch);
        MinHeight = FMath::Min(MinHeight, Point.AltitudeQ12Km);
        MaxHeight = FMath::Max(MaxHeight, Point.AltitudeQ12Km);
    }
    if (MinRelease == MAX_uint32 || MaxRelease <= MinRelease) return LayerId + 1;
    if (MaxHeight <= MinHeight) MaxHeight = MinHeight + 1;

    const auto ScreenPoint = [&](uint32 Release, int32 Height)
    {
        const float X = static_cast<float>(static_cast<double>(Release - MinRelease) / static_cast<double>(MaxRelease - MinRelease)) * Size.X;
        const float Y = Size.Y - static_cast<float>(static_cast<double>(Height - MinHeight) / static_cast<double>(MaxHeight - MinHeight)) * Size.Y;
        return FVector2D(X, Y);
    };
    TArray<FVector2D> Onboard;
    TArray<FVector2D> Ground;
    for (const FKsa64OperationsReleasePoint& Point : History)
    {
        if (!Point.bHasPosition) continue;
        Onboard.Add(ScreenPoint(Point.ReleaseEpoch, Point.PositionQ12[2]));
        if (Point.bHasGroundEstimate) Ground.Add(ScreenPoint(Point.ReleaseEpoch, Point.GroundPositionQ12[2]));
    }
    TArray<FVector2D> Predicted;
    for (const FKsa64OperationsPredictionPoint& Point : Prediction)
        Predicted.Add(ScreenPoint(Point.ReleaseEpoch, Point.AltitudeQ12Km));

    if (Onboard.Num() >= 2)
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 2, AllottedGeometry.ToPaintGeometry(), Onboard, ESlateDrawEffect::None, Cyan, true, 2.2f);
    if (Ground.Num() >= 2)
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 3, AllottedGeometry.ToPaintGeometry(), Ground, ESlateDrawEffect::None, Amber, true, 1.5f);
    if (Predicted.Num() >= 2)
        FSlateDrawElement::MakeLines(OutDrawElements, LayerId + 4, AllottedGeometry.ToPaintGeometry(), Predicted, ESlateDrawEffect::None, Green, true, 1.5f);
    return LayerId + 4;
}

void SKsa64OperationsDashboard::Construct(const FArguments& Args)
{
    Subsystem = Args._Subsystem;

    ChildSlot
    [
        SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(Background)
        .Padding(FMargin(18.0f))
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot()
            .AutoHeight()
            [
                BuildHeader()
            ]
            + SVerticalBox::Slot()
            .AutoHeight()
            .Padding(0.0f, 10.0f, 0.0f, 12.0f)
            [
                BuildTransportControls()
            ]
            + SVerticalBox::Slot()
            .FillHeight(1.0f)
            [
                SNew(SSplitter)
                .PhysicalSplitterHandleSize(6.0f)
                + SSplitter::Slot()
                .Value(0.61f)
                [
                    SNew(SVerticalBox)
                    + SVerticalBox::Slot()
                    .FillHeight(0.58f)
                    [
                        BuildTrajectoryPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.42f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        SNew(SSplitter)
                        + SSplitter::Slot()
                        .Value(0.54f)
                        [
                            BuildNavigationPanel()
                        ]
                        + SSplitter::Slot()
                        .Value(0.46f)
                        [
                            BuildTimelinePanel()
                        ]
                    ]
                ]
                + SSplitter::Slot()
                .Value(0.39f)
                [
                    SNew(SVerticalBox)
                    + SVerticalBox::Slot()
                    .FillHeight(0.27f)
                    [
                        BuildProcedurePanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.28f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildUplinkPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.22f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildDispositionPanel()
                    ]
                    + SVerticalBox::Slot()
                    .FillHeight(0.23f)
                    .Padding(0.0f, 10.0f, 0.0f, 0.0f)
                    [
                        BuildEngineeringPanel()
                    ]
                ]
            ]
        ]
    ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildHeader()
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor(FLinearColor(0.025f, 0.075f, 0.105f, 1.0f))
        .Padding(FMargin(16.0f, 11.0f))
        [
            SNew(SHorizontalBox)
            + SHorizontalBox::Slot()
            .FillWidth(1.0f)
            .VAlign(VAlign_Center)
            [
                SNew(SVerticalBox)
                + SVerticalBox::Slot()
                .AutoHeight()
                [
                    Label(
                        FText::FromString(TEXT("KSA64  /  MISSION FOUNDRY")),
                        19,
                        White)
                ]
                + SVerticalBox::Slot()
                .AutoHeight()
                [
                    Label(
                        TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::HeaderMissionText),
                        11,
                        Cyan)
                ]
            ]
            + SHorizontalBox::Slot()
            .AutoWidth()
            .HAlign(HAlign_Right)
            .VAlign(VAlign_Center)
            [
                Label(
                    TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::HeaderStateText),
                    12,
                    Green)
            ]
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTransportControls()
{
    return SNew(SHorizontalBox)
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("BEGIN GUIDED OPS")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStart),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanStart),
                Green)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::PauseResumeText),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnPauseResume),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Amber)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("STEP +1")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStep),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Cyan)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("4×")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::FourX),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("16×")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::SixteenX),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(0.0f, 0.0f, 7.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("MAX")),
                FOnClicked::CreateSP(
                    this,
                    &SKsa64OperationsDashboard::OnSetPace,
                    EKsa64OperationsPace::Fastest),
                TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::HasSession),
                Muted)
        ]
        + SHorizontalBox::Slot().FillWidth(1.0f)
        [
            SNew(SSpacer)
        ]
        + SHorizontalBox::Slot().AutoWidth()
        [
            BuildAccessibilityControls()
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTrajectoryPanel()
{
    const TSharedRef<SVerticalBox> Body = SNew(SVerticalBox)
        + SVerticalBox::Slot().AutoHeight()
        [
            Label(
                FText::FromString(
                    TEXT("● ONBOARD ESTIMATE    ● GROUND ESTIMATE    ● GROUND-PROPAGATED PREDICTION")),
                10,
                Muted)
        ]
        + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 9.0f, 0.0f, 0.0f)
        [
            SNew(SKsa64OperationsPlot).Subsystem(Subsystem)
        ];
    return Panel(FText::FromString(TEXT("TRAJECTORY  /  ALTITUDE PROFILE")), Body, Cyan);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildNavigationPanel()
{
    return Panel(
        FText::FromString(TEXT("NAVIGATION")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::NavigationText),
            11,
            PrimaryText()),
        Cyan);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildTimelinePanel()
{
    return Panel(
        FText::FromString(TEXT("OPERATIONS TIMELINE")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::TimelineText),
            10,
            PrimaryText()),
        Amber);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildProcedurePanel()
{
    return Panel(
        FText::FromString(TEXT("ACTIVE PROCEDURE")),
        SNew(SVerticalBox)
        + SVerticalBox::Slot().AutoHeight()
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::ProcedureText),
                14,
                Amber)
        ]
        + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 8.0f, 0.0f, 0.0f)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::ProcedureGuardText),
                10,
                PrimaryText())
        ],
        Amber);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildUplinkPanel()
{
    return Panel(
        FText::FromString(TEXT("UPLINK  /  LOAD–VALIDATE–COMMIT")),
        SNew(SVerticalBox)
        + SVerticalBox::Slot().FillHeight(1.0f)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::UplinkText),
                10,
                PrimaryText())
        ]
        + SVerticalBox::Slot().AutoHeight().Padding(0.0f, 9.0f, 0.0f, 0.0f)
        [
            SNew(SUniformGridPanel).SlotPadding(FMargin(3.0f))
            + SUniformGridPanel::Slot(0, 0)
            [
                CommandButton(
                    FText::FromString(TEXT("1  REVIEW")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnReview),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanReviewAction),
                    Cyan)
            ]
            + SUniformGridPanel::Slot(1, 0)
            [
                CommandButton(
                    FText::FromString(TEXT("2  STAGE")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnStage),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanStageAction),
                    Amber)
            ]
            + SUniformGridPanel::Slot(0, 1)
            [
                CommandButton(
                    FText::FromString(TEXT("3  COMMIT")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnCommit),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanCommitAction),
                    Green)
            ]
            + SUniformGridPanel::Slot(1, 1)
            [
                CommandButton(
                    FText::FromString(TEXT("CANCEL")),
                    FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnCancel),
                    TAttribute<bool>::CreateSP(this, &SKsa64OperationsDashboard::CanCancelAction),
                    Red)
            ]
        ],
        Green);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildDispositionPanel()
{
    return Panel(
        FText::FromString(TEXT("MISSION DISPOSITION")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::DispositionText),
            10,
            Green),
        Green);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildEngineeringPanel()
{
    return Panel(
        FText::FromString(TEXT("ENGINEERING  /  INTEGRITY")),
        Label(
            TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::EngineeringText),
            9,
            Muted),
        Muted);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::BuildAccessibilityControls()
{
    return SNew(SHorizontalBox)
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("TEXT")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnTextScale),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("CONTRAST")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnHighContrast),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("MOTION")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnReducedMotion),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(3.0f, 0.0f)
        [
            CommandButton(
                FText::FromString(TEXT("SOUND")),
                FOnClicked::CreateSP(this, &SKsa64OperationsDashboard::OnSoundCues),
                true,
                Muted)
        ]
        + SHorizontalBox::Slot().AutoWidth().Padding(9.0f, 0.0f, 0.0f, 0.0f).VAlign(VAlign_Center)
        [
            Label(
                TAttribute<FText>::CreateSP(this, &SKsa64OperationsDashboard::AccessibilityText),
                9,
                Muted)
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::Panel(
    const FText& Title,
    const TSharedRef<SWidget>& Content,
    const FLinearColor& Accent) const
{
    return SNew(SBorder)
        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
        .BorderBackgroundColor_Lambda([this]() { return PanelBackground(); })
        .Padding(PanelPadding)
        [
            SNew(SVerticalBox)
            + SVerticalBox::Slot().AutoHeight()
            [
                SNew(SHorizontalBox)
                + SHorizontalBox::Slot().AutoWidth()
                [
                    SNew(SBox)
                    .WidthOverride(4.0f)
                    .HeightOverride(18.0f)
                    [
                        SNew(SBorder)
                        .BorderImage(FCoreStyle::Get().GetBrush(TEXT("WhiteBrush")))
                        .BorderBackgroundColor(Accent)
                    ]
                ]
                + SHorizontalBox::Slot().FillWidth(1.0f).Padding(9.0f, 0.0f)
                [
                    Label(Title, 11, Accent)
                ]
            ]
            + SVerticalBox::Slot().FillHeight(1.0f).Padding(0.0f, 10.0f, 0.0f, 0.0f)
            [
                Content
            ]
        ];
}

TSharedRef<SWidget> SKsa64OperationsDashboard::Label(
    TAttribute<FText> Text,
    int32 BaseSize,
    const FLinearColor& Color) const
{
    return SNew(STextBlock)
        .Text(Text)
        .ColorAndOpacity(Color)
        .Font_Lambda([this, BaseSize]()
        {
            return FCoreStyle::GetDefaultFontStyle(
                TEXT("Regular"),
                FMath::RoundToInt(BaseSize * TextScale()));
        })
        .AutoWrapText(true);
}

TSharedRef<SWidget> SKsa64OperationsDashboard::CommandButton(
    TAttribute<FText> Text,
    const FOnClicked& OnClicked,
    TAttribute<bool> Enabled,
    const FLinearColor& Accent) const
{
    return SNew(SButton)
        .ButtonColorAndOpacity(FLinearColor(Accent.R * 0.20f, Accent.G * 0.20f, Accent.B * 0.20f, 1.0f))
        .ForegroundColor(Accent)
        .ContentPadding(FMargin(10.0f, 6.0f))
        .IsEnabled(Enabled)
        .OnClicked(OnClicked)
        [
            SNew(STextBlock)
            .Text(Text)
            .Font(FCoreStyle::GetDefaultFontStyle(TEXT("Bold"), 10))
        ];
}

FReply SKsa64OperationsDashboard::OnKeyDown(
    const FGeometry& MyGeometry,
    const FKeyEvent& InKeyEvent)
{
    const FKey Key = InKeyEvent.GetKey();
    if (Key == EKeys::SpaceBar) return OnPauseResume();
    if (Key == EKeys::Period) return OnStep();
    if (Key == EKeys::One) return OnSetPace(EKsa64OperationsPace::Realtime);
    if (Key == EKeys::Four) return OnSetPace(EKsa64OperationsPace::FourX);
    if (Key == EKeys::Zero) return OnSetPace(EKsa64OperationsPace::Fastest);
    return SCompoundWidget::OnKeyDown(MyGeometry, InKeyEvent);
}

FReply SKsa64OperationsDashboard::OnStart()
{
    if (Subsystem.IsValid())
    {
        Subsystem->StartGuidedOperations();
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnPauseResume()
{
    if (Subsystem.IsValid())
    {
        if (Subsystem->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused)
        {
            Subsystem->ResumeRealtime();
        }
        else
        {
            Subsystem->PausePresentation();
        }
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnStep()
{
    if (Subsystem.IsValid())
    {
        Subsystem->StepOneRelease();
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnSetPace(EKsa64OperationsPace Pace)
{
    if (Subsystem.IsValid())
    {
        Subsystem->SetPace(Pace);
    }
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnReview()
{
    if (Subsystem.IsValid()) Subsystem->ReviewAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnStage()
{
    if (Subsystem.IsValid()) Subsystem->StageAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnCommit()
{
    if (Subsystem.IsValid()) Subsystem->CommitAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnCancel()
{
    if (Subsystem.IsValid()) Subsystem->CancelAction();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnReducedMotion()
{
    if (Subsystem.IsValid()) Subsystem->ToggleReducedMotion();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnHighContrast()
{
    if (Subsystem.IsValid()) Subsystem->ToggleHighContrast();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnSoundCues()
{
    if (Subsystem.IsValid()) Subsystem->ToggleSoundCues();
    return FReply::Handled();
}

FReply SKsa64OperationsDashboard::OnTextScale()
{
    if (Subsystem.IsValid()) Subsystem->CycleTextScale();
    return FReply::Handled();
}

FText SKsa64OperationsDashboard::HeaderMissionText() const
{
    if (!Subsystem.IsValid())
    {
        return FText::FromString(TEXT("OPERATIONS SUBSYSTEM UNAVAILABLE"));
    }
    return FText::FromString(FString::Printf(
        TEXT("KSA-G10R  ·  GNSS LOSS  ·  %s  ·  %s"),
        *Subsystem->GetMissionElapsedLabel().ToString(),
        *Subsystem->GetReleaseLabel().ToString()));
}

FText SKsa64OperationsDashboard::HeaderStateText() const
{
    if (!Subsystem.IsValid())
    {
        return FText::FromString(TEXT("OFFLINE"));
    }
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s   |   %s   |   %s   |   %s"),
        *View.RoleLabel,
        *View.FrameLabel,
        *Subsystem->GetPaceLabel().ToString(),
        *View.SessionStatus));
}

FText SKsa64OperationsDashboard::NavigationText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s\n\nONBOARD POSITION Q12  %+d  %+d  %+d\nGROUND POSITION Q12   %+d  %+d  %+d\n\nONBOARD VELOCITY Q24  %+d  %+d  %+d\nGROUND VELOCITY Q24   %+d  %+d  %+d\n\n%s"),
        *View.NavigationLabel,
        View.NavigationPositionQ12[0], View.NavigationPositionQ12[1], View.NavigationPositionQ12[2],
        View.GroundPositionQ12[0], View.GroundPositionQ12[1], View.GroundPositionQ12[2],
        View.NavigationVelocityQ24[0], View.NavigationVelocityQ24[1], View.NavigationVelocityQ24[2],
        View.GroundVelocityQ24[0], View.GroundVelocityQ24[1], View.GroundVelocityQ24[2],
        *View.CommunicationsLabel));
}

FText SKsa64OperationsDashboard::ProcedureText() const
{
    return Subsystem.IsValid()
        ? FText::FromString(Subsystem->GetViewModel().ProcedureLabel)
        : FText::GetEmpty();
}

FText SKsa64OperationsDashboard::ProcedureGuardText() const
{
    return Subsystem.IsValid()
        ? FText::FromString(Subsystem->GetViewModel().ProcedureGuard)
        : FText::GetEmpty();
}

FText SKsa64OperationsDashboard::UplinkText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("%s\n\n%s\n\nActions are accepted only as Rust-generated typed proposals. "
             "No direct effector command and no K-format parsing exists here."),
        *View.UplinkLabel,
        *View.ActionReceiptLabel));
}

FText SKsa64OperationsDashboard::TimelineText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const TArray<FKsa64OperationsTimelineItem>& Items = Subsystem->GetTimeline();
    FString Text;
    const int32 First = FMath::Max(0, Items.Num() - 7);
    for (int32 Index = First; Index < Items.Num(); ++Index)
    {
        const FKsa64OperationsTimelineItem& Item = Items[Index];
        Text += FString::Printf(
            TEXT("%06u  %-10s  %s%s"),
            Item.ReleaseEpoch,
            *Item.Category.Left(10),
            Item.bAttention ? TEXT("◆ ") : TEXT("· "),
            *Item.Summary);
        if (Index + 1 < Items.Num()) Text += TEXT("\n");
    }
    return FText::FromString(Text.IsEmpty() ? TEXT("No operational events observed") : Text);
}

FText SKsa64OperationsDashboard::DispositionText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    if (!View.Capabilities.bDisposition)
    {
        return FText::FromString(
            TEXT("MISSION       —\nVEHICLE       —\nPROCEDURE     —\nOPERATOR      —\nAVIONICS      —\nEVIDENCE      —\n\n"
                 "Awaiting Rust-derived disposition view; procedure conformance is not mission success."));
    }
    return FText::FromString(View.DispositionLabel);
}

FText SKsa64OperationsDashboard::EngineeringText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsViewModel& View = Subsystem->GetViewModel();
    return FText::FromString(FString::Printf(
        TEXT("BRIDGE      %s\nROLE FILTER %s\nPUBLICATION %llu / %d\n"
             "QUEUES      CMD %u/%u  EVENT %u/%u  SAMPLE %u/%u\n"
             "WORKER      %u  FINALIZE %u  OVERFLOW %u\n"
             "EVIDENCE    %08X  %llu bytes  CRC %08X\n"
             "STATUS      %s\n"
             "CHECKSUMS   %08X  %08X  %08X\nOBSERVED    %s\nDIAGNOSTIC  %s"),
        *View.BridgeStatus,
        View.bTruthFiltered ? TEXT("TRUTH FILTERED") : TEXT("SIM DIRECTOR TRUTH"),
        static_cast<unsigned long long>(View.CommandSequence),
        View.CommandResult,
        View.CommandsPending, View.CommandCapacity,
        View.TimelinePending, View.TimelineCapacity,
        View.SamplesPending, View.SampleCapacity,
        View.WorkerState, View.FinalizationState, View.TransportOverflow,
        View.EvidenceIdentity, static_cast<unsigned long long>(View.EvidenceLength), View.EvidenceCrc32,
        *View.EvidenceStatus,
        View.FlightChecksum,
        View.NavigationChecksum,
        View.CommandChecksum,
        View.bObservationComplete ? TEXT("COMPLETE PREFIX") : TEXT("BOUNDED PREFIX"),
        *View.LastDiagnostic));
}

FText SKsa64OperationsDashboard::PauseResumeText() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().PresentationPace == EKsa64OperationsPace::Paused
        ? FText::FromString(TEXT("RESUME 1×"))
        : FText::FromString(TEXT("PAUSE"));
}

FText SKsa64OperationsDashboard::AccessibilityText() const
{
    if (!Subsystem.IsValid()) return FText::GetEmpty();
    const FKsa64OperationsAccessibilitySettings& Access = Subsystem->GetAccessibility();
    return FText::FromString(FString::Printf(
        TEXT("%.0f%%  ·  %s  ·  %s  ·  %s"),
        Access.TextScale * 100.0f,
        Access.bHighContrast ? TEXT("HIGH CONTRAST") : TEXT("STANDARD"),
        Access.bReducedMotion ? TEXT("REDUCED MOTION") : TEXT("SMOOTH"),
        Access.bSoundCues ? TEXT("CUES ON") : TEXT("CUES OFF")));
}

bool SKsa64OperationsDashboard::HasSession() const
{
    return Subsystem.IsValid() && Subsystem->GetViewModel().bSessionOpen;
}

bool SKsa64OperationsDashboard::CanStart() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().bBridgeReady
        && !Subsystem->GetViewModel().bSessionOpen;
}

bool SKsa64OperationsDashboard::CanReviewAction() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().bSessionOpen
        && Subsystem->GetViewModel().Capabilities.bTypedActions
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Available;
}

bool SKsa64OperationsDashboard::CanStageAction() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Reviewing;
}

bool SKsa64OperationsDashboard::CanCommitAction() const
{
    return Subsystem.IsValid()
        && Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Staged
        && Subsystem->GetViewModel().ReleaseEpoch >= Subsystem->GetViewModel().ActionEarliestCommitEpoch;
}

bool SKsa64OperationsDashboard::CanCancelAction() const
{
    return Subsystem.IsValid()
        && (Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Staged
            || Subsystem->GetViewModel().ActionState == EKsa64OperationsActionState::Committed);
}

float SKsa64OperationsDashboard::TextScale() const
{
    return Subsystem.IsValid() ? Subsystem->GetAccessibility().TextScale : 1.0f;
}

FLinearColor SKsa64OperationsDashboard::PanelBackground() const
{
    return Subsystem.IsValid() && Subsystem->GetAccessibility().bHighContrast
        ? PanelHighContrast
        : PanelDark;
}

FLinearColor SKsa64OperationsDashboard::PrimaryText() const
{
    return Subsystem.IsValid() && Subsystem->GetAccessibility().bHighContrast
        ? White
        : FLinearColor(0.74f, 0.82f, 0.88f, 1.0f);
}

